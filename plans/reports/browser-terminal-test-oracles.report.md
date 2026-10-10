## What this was about

Nine terminal-related tests could pass without proving the behavior they claimed to protect: recovery despite empty
frames or view switching, an unanswered heartbeat, link dragging, deletion without confirmation, a closed tab staying
closed, exact mouse bytes, and console capture after a hostile message. You chose to repair all nine in one draft PR,
leaving any unexpectedly complex outcome for triage.

## Things you should know

All nine repairs stayed in the existing tests and their observation facilities. No product behavior, specification or
production hook changed, and no outcome was dropped. Their feedback files and complete index entries are removed; all
nine completion records point to the same PR.

The recovery tests now require recovery while the challenging traffic or view switching is still happening. The
heartbeat case counts a probe sent on the actual terminal connection after replies are silenced, so a previously
answered probe cannot satisfy its premise. Both deletion cases prove that the real request reached its hold, including
its guard query, and release that hold on failure.

The closed-tab test now observes application of an older session refresh before allowing a newer refresh to repair the
result. It then observes application of a refresh started after the close. Each check reads the tab count once, so a
transient resurrection cannot disappear inside an assertion retry. Source review caught the masking effect of the newer
refresh and the incomplete-output readiness in the link test; both were corrected.

Two initial failures exposed incorrect fixture assumptions rather than product failures: the mouse click was measured
against the terminal's outer box instead of its cell area, and the supposedly single-row drag URL wrapped. The fixtures
were corrected within the agreed scope. Mouse reports are now compared as the complete exact press/release byte stream,
the link check waits for the whole URL on one row, and the console test flushes and checks the later message's actual
delivery instead of accepting an original-console call as capture evidence.

## Open questions and possible follow-ups

An unchanged rollback/reconnect case has a latent fixture race: its simulated older-helm reply can arrive before the
first retry, correctly putting the page in manual-only mode while the test still expects an automatic attempt. It failed
once in the complete selected-spec run and once in three exact WebKit repetitions. The trace establishes the premature
mismatch; identical-source passes establish intermittence. FLAKES.md and an open Deflake TODO entry preserve the
evidence. The recommended follow-up is to stage the mismatch after the first witnessed attempt, while continuing to
observe ordinary session connections that would replace the current terminal owner. This PR records the finding without
repairing or suppressing that unrelated case.

Browser execution is on Linux using Chromium and WebKit; WebKit covers the desktop engine family, not a native macOS
application run. No agreed outcome was deferred and no decision is needed to land the nine repairs.

## PRs

- [#1819 — reject false passes in terminal checks](https://github.com/scode/farhelm/pull/1819/changes), one draft PR for
  all nine outcomes. The executor has not marked it ready or merged it.

## Checks run, reused and skipped

Complete selected-spec run `75991cc8-e095-4d86-a1c2-483aac3053db` finished with 313 passes and one failure in 48
minutes: Chromium 157/157, WebKit 156/157. Every directly changed browser case passed on both engines. There were no
skipped, interrupted or unstarted cases, and cleanup completed normally. The unchanged rollback fixture described above
was the only unexpected failure; this is a red run, not a 314/314 pass.

Runtime checks used `python3 scripts/record-test-run.py`. The final required browser selection is the six complete
changed specs: `mouse-modes.spec.ts`, `terminal-links.spec.ts`, `terminal-reconnect.spec.ts`,
`terminal-replay-rename.spec.ts`, `terminal-tabs.spec.ts` and `terminal.spec.ts`, on Chromium and WebKit with one worker
and zero retries. The recorder enforces the selection's strict policy and retains the full result.

Exact rollback reproduction used generic recording and
`npx playwright test --project=webkit-terminal-reconnect -g 'a-rolled-back-helm-gets-no-automatic-attach' --workers=1 --retries=0 --repeat-each=3`.
Run `2a5f5e3b-5aab-42df-a844-6b7e196094fc` had one failure and two passes, with normal cleanup. Its JSON results and
attachments are retained. An earlier reproduction command, run `d9b30c6b-565d-4765-b88c-a6167e7dc6b5`, matched no tests
because its anchored selector did not match the fully qualified Playwright title; it is an invocation failure with no
runtime observation. The full failed run is retained separately and is not replaced by the narrow passes.

The UI JavaScript harness (`node --test` from its test directory) passed 226/226, run
`04ec990a-7f7a-4206-9a54-cc92862f67d0`. It covered the repaired console test over base `d8d43e77`; that file has not
changed since, so its evidence remains applicable. `cargo build -j 4` and the Dioxus release web build passed and were
reused throughout because product source never changed. Playwright preparation matched the locked version 1.64.0 and
browser execution used pinned tmux 3.7c.

Initial exact Chromium execution, run `1cf8b15e-9dad-4872-881e-eeebe92a12fa`, passed seven cases and failed the mouse
geometry and wrapped-link premises described above. Their narrow corrected run `275de45c-96f3-45dc-be80-ba811c9ba230`
passed both. After the source review corrections, exact Chromium tab/link run `4e3c9bbe-ecbb-46d2-8e4a-b24a1f8476b1`
passed both. Failed evidence remains retained; later passes do not replace it.

Deliberate controls were rejected where intended. A stuck console capture guard failed the new delivery assertion while
the original-console assertions passed, run `1bb33ec6-a4c7-4e2f-b3d6-3197690d1b40`. Browser control run
`b199f1c8-da9c-434d-8619-f8d1c12da5a5` produced three intended failures: UTF-8-expanded mouse coordinates, progress
frames preventing the silence watchdog's reveal, and an intentional failure after held deletion arrived. The last
completed teardown without timeout, proving release on failure. The original wrapped URL also served as a rejected
fixture control. All temporary controls were restored; none changed production source.

Additional product mutations for heartbeat, reconnect scheduling and tab retirement were omitted: their state is private
to the shipped browser code, and exercising those mutations would require another build or new mechanisms. The tests
instead directly witness post-silence sends, continuous churn, applied refresh ordering and request-handler arrival.

An earlier full selected-spec run, `9e7bb531-b51b-493b-af18-0c2fe61a75b3`, was deliberately interrupted before fixing
the review findings: 15 passed, one interrupted and 298 unstarted, exit 130. The recorder eventually forced cleanup of
its owned process group. It is retained as incomplete evidence and is not counted as a pass.

The isolated source-only delay checker inspected 282 delays with zero missing reasons. Formatting of the completion
ledger, feedback index, flake history and TODO passed. Repository formatting does not cover these JavaScript/TypeScript
files; their localized style and documentation were inspected separately.

Every main diff from `d8d43e77` through `264a2800` was read: queue transitions, reports, Settings TODO entries, new
checkout-folder/UI planning and unattended-execution instructions. They do not affect executable code, fixtures or
specifications. The conflict-free careful rebase preserved all seven test files byte-for-byte relative to tested head
`6494688b`; later changes add only the flake history and follow-up record. Builds, successful JS execution, the complete
browser observations and the source review therefore remain applicable. No repeat runtime check was needed for these
documentation and ancestry changes.

The full browser suite, workspace Rust runtime, desktop application, installer and release checks were skipped: the plan
requires these six changed specs and the JS harness, and no product code changed. Hosted CI was not dispatched. No
changelog fragment is required for this test-only change.

## Review gate outcome

The prescribed fresh native reader requested on gpt-6.1-sol at high effort received the full charter, all nine ledger
contracts and the verbatim test-authoring checklist. It identified the masked tab resurrection and partial link-output
readiness; both were accepted and fixed. Its follow-up closed both findings and found no new material issues. The
executor assessed runtime evidence separately; source review did not run tests.

The second fresh wording reader requested on gpt-6.1-sol at medium effort understood the problems and found no unclear
or contradicted claims or convention violations. A separate reader confirmed that the resume log agreed with the plan
and PR state. Implementation and investigation stayed in the executing session, as the plan required. Actual native
model attribution and usage counters were unavailable; requested routes are recorded as requests in private session
`d0e361cc-2cf7-49dd-a656-7f0f6a284fbf` and the plan's working log beside the checkouts.
