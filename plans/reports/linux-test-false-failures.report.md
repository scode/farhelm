## What this was about

Twelve triaged findings concerned tests for session launches, Stop and Restart, host uninstall, tmux adoption and file
uploads. Correct product behavior could fail a test when a directory report was only partly written, the system shell
replaced its final command, paths needed quoting, a notification arrived before a reply, or a developer's
state-directory setting reached a fixture. Concurrent Stop tests shared a process marker. A refused HTTP Stop test could
finish observing before the request reached the refusal boundary. Failed assertions could strand stopped processes, a
private tmux server or a systemd unit; successful upload-memory checks left their 64 MiB fixture behind.

The maintainer asked for these twelve small repairs together in one draft PR, leaving any outcome that required
substantially more complexity for further triage. All twelve assessments still held, and all twelve were repaired within
their named tests and fixtures.

## Things you should know

Directory witnesses now require a complete newline-terminated report before comparison. The fake agent retains the
intermediate shell needed to exercise descendant cleanup, Stop cancellation uses a fresh process marker, and fixture
children receive an isolated state directory. Generic launches use the fixture's actual argument encoding, and the raw
Restart helper waits for the matching request reply rather than accepting unsolicited notifications.

The foreign-origin Stop test observes continuously through the HTTP response and retains its original two-second silence
window afterward. That window starts after the response so setup cannot exhaust it, and the same pending read survives
the response boundary. A frame, transport error or EOF fails the check. This remains a bounded observation, not a
scheduler-independent proof that no arbitrarily delayed frame can arrive.

Cleanup is owned before the fallible setup steps: the real unit before activation assertions, the old tmux server before
startup, and the stopped child immediately after spawning. The upload-memory child explicitly removes its directory
after measurement and before direct exit. Product assertions and measurement thresholds remain intact. No production
hook, fixture redesign or product behavior change was introduced.

All twelve ledger entries reference the same change and draft PR. Their feedback files and whole index entries are
removed. No outcome was discarded or deferred by its complexity gate. The separate portability plan's page-size
measurement change was left to that plan.

## Open questions and possible follow-ups

None. The frequency of the partial-report race, early notifications and shared-marker interference was not measured;
source inspection established the faulty boundaries. No production failure frequency is claimed. macOS runtime behavior
and large-page Linux measurement remain outside this Linux fixture repair.

## PRs

- [#1814 — prevent false test failures and fixture leaks](https://github.com/scode/farhelm/pull/1814/changes), one draft
  PR based on main.

## Checks run, reused and skipped

- Passed: `cargo clippy --all-targets -- -D warnings`, workspace Rust formatting, changed Markdown formatting, and the
  isolated-interpreter test-delay checker (282 delays, zero missing rationales). The first Clippy attempt identified two
  nested conditions to collapse; their equivalent let-chain spelling passed the rerun. Runtime evidence is reused across
  those spelling changes because the readiness predicates and deadlines are unchanged.
- Focused recorded nextest run `5d440bf3-0a01-4f6d-93d9-cca816df44d4` passed all 13 selected tests, with pinned nextest
  and tmux, four slots and zero retries. It includes the named launch, Stop, Restart, uninstall and cleanup subjects
  plus wrapper Stop and live Restart callers. Real systemd and below-floor/current tmux substrates ran; selected tests
  emitted no runtime skip. Twelve unchanged results are reused; its foreign-origin result predates the
  observation-window correction and is superseded by the restored run below.
- Restored recorded nextest run `9b854256-4ba2-4237-b1e9-980f8508d6ae` passed all six selected tests, with pinned
  nextest and tmux, four slots and zero retries; no selected test reported a runtime skip. It covers the corrected
  foreign-origin observer, real-unit unwind, old-server cleanup, stopped-child cleanup, upload-memory cleanup and
  uninstall-command tests. An inherited absolute state-directory setting is supplied only to the launched command; the
  test process does not mutate its own environment.
- Recorded Bash topology proof `c8c22f4d-b8b9-4a8b-a644-b125b339a058` passed: the former final sleep replaces Bash,
  while a trailing command retains the shell and its sleep child.
- Deliberate temporary failure run `70ecbd4c-f895-4631-af8e-5ea6364e8536` produced exactly four expected failures and
  one upload-memory pass. Assertions forced stopped-child, old-tmux and real-unit unwind; their exact owned resources
  were gone, inactive or removed afterward. Delayed Stop forwarding was detected even though the real HTTP response
  remained 403. These intentional failures are retained as mutation evidence, not latent flakes. Every temporary source
  edit was restored before final validation.
- Uncaptured upload-memory proof `7238fad5-9263-4191-a167-5e3a909fb867` passed, and the exact printed fixture directory
  was absent after child exit. Cleanup occurred after measurement.
- Source inspection covers the report-write race, reply correlation, UUID isolation and argument serialization. Forcing
  these with new fixture coordination would exceed the named repairs; no new test seam was added.
- The careful rebase inspected all intervening implementation diffs, including host-text escaping, Git environment
  isolation, OS readback, harness tooling, SSH configuration, documentation and capture changes. The tmux probe edits
  are separate from adoption, the process argument readback is macOS-only, and the fixture's simple unit still matches
  the unit parser. Later documentation/capture and queue changes do not alter these fixtures. Both sets of shared
  bookkeeping were preserved; validation coverage is unchanged.
- Full workspace runtime, browser, desktop, installer and release suites were skipped because this plan changes the
  named tests and fixtures and has focused execution and explicit cleanup proofs. No concrete broader integration gap
  was identified. Hosted CI was not dispatched; draft updates do not start CI in this repository.

## Review gate outcome

The prescribed fresh gpt-6.1-sol high source reviewer inspected all twelve ledger contracts, the final source diff and
the full test-authoring checklist. It confirmed that guards also clean up on success and no product assertion was
weakened. An initial review finding concerned the asynchronous relay after an HTTP response; the executor also
identified it, retained the original post-response silence budget, and obtained a final PASS. The reviewer inspected
source and bookkeeping; the executor verified runtime evidence separately.

The second fresh gpt-6.1-sol medium wording reader understood the motivation and found the commit and PR claims accurate
and conventions satisfied. Implementation stayed local under no-workhorse mode. Actual native model identity and usage
counters were unavailable; requested routes are recorded as requests rather than proof of runtime attribution. Private
evidence is retained under session UUID `a63f6b2c-45f7-4830-bf84-ea76de63472d`. The executor left the PR in draft and
did not merge it.
