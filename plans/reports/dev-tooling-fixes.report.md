## What this was about

Nine triaged bugs affected the tools used to develop and validate Farhelm. The CentOS provisioning test could
temporarily change how unrelated SSH settings applied. Private tmux builds could fail on stock macOS Bash. The plans
watcher could miss available work when main changed between its reads, and answering a blocked plan could leave question
text behind. Test evidence could name the wrong checkout or lose an already observed child result when publication
failed. The terminal handoff probe could pass despite early output, the release-note sweep could accept files that
release curation ignores, and an optional desktop check could time out during normal session shutdown.

The maintainer asked for these nine small repairs in one draft PR, with any unexpectedly complex outcome left for
further triage. All nine assessments still held and all nine were fixed within the named scripts and existing
self-tests.

## Things you should know

The SSH test's removable block now restores global scope before the original configuration. Its own scalar connection
options still take first-match priority. Optional build arrays use the empty-safe expansion compatible with Bash 3.2.
The plans watcher resolves one commit per poll for both its tree and eligibility check; the queue refuses heading
spellings that could split a blocked question into another section.

Checkout discovery removes only Git's terminating newline, preserving whitespace that belongs to a directory name. A
failed first evidence publication keeps the child's exit status, duration, cleanup facts and output-completion
observation available to the fallback record. The recorder still returns its own error status when publication fails;
preserving the child result does not convert that recorder failure into a successful run.

The terminal handoff probe now rejects output notifications anywhere before the final refresh reply, including between
replies. Release-note coverage counts only immediate Markdown fragments, excluding README, and applies that rule to
rename destinations. Optional desktop session deletion uses the default leg's 30-second budget, allowing the
supervisor's existing five-second graceful-stop period and response.

All nine ledger entries reference the same draft PR; their feedback files and complete index entries are removed. No
outcome was discarded or deferred. These are development-tool fixes and do not change shipped Farhelm behavior; the
changelog fragment records that scope.

## Open questions and possible follow-ups

None requiring a decision. Stock macOS Bash 3.2 was unavailable for execution, so portability rests on the documented
expansion idiom, source review and exact expansion probes on current Bash. No stock-Mac pass is claimed. Existing
ShellCheck diagnostics in two scripts remain unchanged, as detailed below.

## PRs

- [#1815 — preserve development checks and evidence](https://github.com/scode/farhelm/pull/1815/changes), one draft PR
  for all nine fixes, based on main.

## Checks run, reused and skipped

Runtime commands below ran through the test recorder; UUIDs identify retained evidence.

| Check                                                                                                   | Result and reason                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `python3 scripts/test-plans-queue.py`                                                                   | 63 tests passed; heading refusal and block/answer round-trip included. Run `4349046f-fa57-4f17-a233-bf47f53ac168`.                                                                                                                                                                                                                   |
| `bash scripts/test-plans-watch.sh`                                                                      | All scenarios passed, including main advancing between reads and commit-lookup failure. Run `ac8aa765-8528-4954-b63f-b059b7d2f0d2`.                                                                                                                                                                                                  |
| `python3 scripts/test-record-test-run.py`                                                               | 56 tests passed, including real whitespace-suffixed checkout discovery and injected first terminal-write failures. Run `763f383c-8ea8-4b7d-829b-1ea35401e9de`.                                                                                                                                                                       |
| `python3 releasing/check-changelog.py --self-test`                                                      | Passed, including ignored files and renames across the fragment-discovery boundary. Run `4688b1fe-c186-4e93-9e29-5b6159b66131`.                                                                                                                                                                                                      |
| Synthetic terminal events, actual SSH-block insertion with `ssh -G`, exact build-array expansion probes | Passed; cover early output, unrelated SSH globals and fixture priority, and empty/populated arguments. Run `ed1fe159-bbb9-4f7b-a29b-655b1c46fbdb`.                                                                                                                                                                                   |
| Stricter handoff probe against pinned tmux 3.7c                                                         | One trial passed. Run `639c47eb-6663-4aa6-9706-e2262171c0b6`.                                                                                                                                                                                                                                                                        |
| Original-source controls loaded in memory                                                               | New assertions deliberately rejected the old recorder and queue; the old terminal probe falsely accepted intervening output. Control driver passed, run `1b28bfa0-3e65-4640-b670-fdd752a40d72`. Live sources were untouched; expected failures are retained as controls.                                                             |
| ShellCheck and desktop Bash syntax                                                                      | Strict ShellCheck passed the CentOS, watcher and watcher-test scripts. Private-build `SC1090` and desktop `SC1091`, `SC2317`, `SC2016` diagnostics match the unchanged base's code/message multisets. Excluding only those baseline codes passes both scripts. Desktop `bash -n` passed. This is not an unqualified ShellCheck pass. |
| Changed Markdown formatting and changelog format                                                        | Passed; the fragment lint checked 43 fragments.                                                                                                                                                                                                                                                                                      |

The initial direct queue invocation named a nonexistent test class and failed discovery; two direct recorder regressions
and a changelog self-test ran before the full recorder-backed suites. Those direct invocations were a process lapse, not
retained execution evidence. The complete recorded suites above establish the reported passes.

The successful checks cover the source reviewed on base `0e351a09`. Main then changed queue claims and another plan's
report only; every intervening diff was read and the rebase preserved this source. Existing runtime results remain
applicable. Subsequent ledger URL bookkeeping requires formatting only.

The full CentOS container test and desktop smoke were skipped as the plan explicitly directs; the SSH configuration was
inspected through OpenSSH itself, and the optional deletion budget matches the existing default leg. Rust, browser,
installer and full workspace batteries were skipped because no product code or those tests changed and targeted checks
cover the changed contracts. The Rust/browser delay checker does not apply to these files. Hosted CI was not dispatched.

## Review gate outcome

The prescribed fresh native reader requested on gpt-6.1-sol high inspected all nine outcome contracts, the source diff
and the full test-authoring checklist. It found no actionable issues, including the SSH first-match contract. Runtime
results were verified separately by the executor. Implementation stayed local under no-workhorse mode.

Commit and PR wording received three fresh native cold reads requested on gpt-6.1-sol medium. Earlier passes found
accurate claims but exposed vague references and an inventory of changes. The final candidate states the underlying
failures; its reader understood the motivation, found claims accurate and found no convention defects. One sentence
identified as redundant was removed. The final commit and PR use that reviewed wording.

Native tools did not expose actual model identity or usage counters. Requested routes are recorded as requests, not
runtime attribution. Private routing evidence is retained under session UUID `d0e361cc-2cf7-49dd-a656-7f0f6a284fbf`. The
executor leaves the PR in draft and does not merge it.

### Landing

Landed on 2026-10-10 (UTC) as #1815. The rebase met only the review queue index, where each plan removes its own
entries.

#### Review before merging

A separate reviewer read the plans queue and watcher changes in depth against `plans/AGENTS.md`: the queue's state
machine, publish and exit codes are unchanged; the watcher resolves main's commit once per poll and uses it for both the
tree read and the wake check, which fixes the race this plan targeted; the recorder changes keep its documented
contracts.

#### A fix made while landing

The watcher looked up main's commit through GitHub's commit endpoint, which also returns the commit's whole file list
and patches; a very large commit on main could make every poll fail, and after three failures the executors and the
lander would stop waiting with an error. The landing switched it to the branch-ref endpoint, which returns only the id,
and updated the watcher test's stand-in. In #1815 before it merged.

#### Things to know

The watcher's own test script fails now and then under heavy load, in the case that expects a change found on the last
poll before the deadline: 2 of 8 runs on main's copy and 4 of 16 with this change, at load averages around 12 to 20 on
18 CPUs. One branch run also failed once with an "unbound variable" message that did not recur. Neither was logged as a
flake. Smaller notes left as they are: when the commit lookup fails, the error message still names the tree request; a
malformed commit answer has no test; and the queue's heading check now also refuses a `---` or `===` line right after an
ATX heading, a quote or a list item, where Markdown would read it as a separator, so a decision text may need rewording
before `answer` or `follow-up` accept it.

#### Checks

These six plans were landed together, stacked in landing order on main after gui-text-safety, ui-interaction-fixes and
helm-cli-fixes merged; browser-terminal-test-oracles landed after them in the same round.

- Run now, on the six stacked: `cargo fmt --all -- --check`, `dprint check`, the changelog lint, the plans-queue tests,
  the test-recorder tests, the installer tests (517 checks), `sh -n scripts/install.sh`,
  `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`; the supervisor, helm,
  UI, protocol and CLI unit tests with the agent-relay, upload, boot-outcome, hook-identity, restart-with-resume and
  session-lifecycle end-to-end tests, through the recorder with pinned tmux 3.7c, four slots and no retries (run
  `8bc6001f`, 3064 of 3064); the UI JavaScript tests (230 of 230); and on Chromium and WebKit with one worker and no
  retries the notifications, sidebar-resize, sidebar, terminal-attachments, terminal-create-idempotency,
  terminal-multihost, yolo-guard, templates and quick-switcher specs (run `77ec2061`, 472 passed). `shellcheck` reports
  the same warnings for `scripts/build-private-tmux.sh` and `scripts/desktop-smoke.sh` on main as with these changes.
- These plans were claimed while an earlier round was still landing, which delayed their merge; the landing instructions
  were changed the same day (#1829) so that a round's plans are fixed when it starts.

Nothing in the report above was made untrue by the landing.
