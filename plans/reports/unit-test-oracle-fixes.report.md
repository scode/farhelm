## What this was about

Thirteen triaged findings concerned tests of the supervisor, the helm and cleanup of abandoned test runs. Some could
pass without exercising their promised behavior; others could fail while a fixture was still starting. The test-run
cleanup could also delete a tmux server's socket directory after an unconfirmed kill, leaving no route for a later
cleanup attempt. The maintainer chose contained test fixes, with findings discarded when strengthening them would need a
new testing hook in product code.

Eleven outcomes were fixed in one draft PR. Two replacement-refusal findings were discarded under that agreed gate. All
thirteen feedback items were removed and their execution records link to the PR. Nothing changes for someone running
Farhelm; the cleanup change affects test-owned tmux servers only.

## Things you should know

The terminal tests now prove that input arrived before checking that an already-answered terminal query was removed; an
empty input stream can no longer pass. Tests of waiting and running session status wait for the prompt or fresh output
in the session they classify, so a slow shell or briefly descheduled printer does not make correct classification fail.

The managed-checkout tests check that a deleted and recreated directory has a distinguishable identity before expecting
Farhelm to refuse it as a replacement. If both its inode and birth time were reused, the test visibly skips that
premise. Reconciliation after a checkout disappears now accepts an absent or empty archive directory while still proving
that no checkout was invented. The helm shutdown test forces host reconciliation to be waiting on its database when
shutdown occurs, rather than assuming one scheduler yield establishes that race. The quiet-logging test observes five
completed host-list refreshes before deciding that a healthy connected host does not produce a reconnection log line
every tick.

Three conversation-report tests now put the competing agent process inside the session's process tree and require the
specific refusal for a nested agent, so an unrelated-process rejection cannot pass them. This guards against a child
agent taking over its parent session's conversation binding. The macOS test of Apple's withholding of a platform
program's environment now proves that the child has started the intended program, that observations succeeded and that
the child is still alive. It was not executed on macOS.

The shared silent host reads from handshake onward and remains connected for a full two-second window after the response
being checked. Frames, read failures and early EOF fail. One focused run exposed a caller that intentionally reconnected
its host before starting that window; the window now finishes immediately after the refusal, before that separate
reconnect phase. The corrected test passed, and the independent reviewer confirmed the phase boundary.

Replace starts a successor session before deleting the original. If the host incorrectly reports the original itself as
the successor, the helm must refuse the operation and preserve that original session. Two unchanged tests check this
refusal, but still do not directly prove that a later delete request cannot reach the host and remove the original.
Shutting down the helm's host connections can cancel a queued or in-progress request, so seeing no delete before the
connection closes would not establish that guarantee. Waiting until every write finishes would require new product
plumbing. Both findings were therefore discarded under the recorded complexity gate, without changing their original
assessment or claiming their tests now prove that guarantee. No current product deletion defect was established.

Test cleanup now retains a socket directory whenever a kill was skipped, timed out or otherwise unconfirmed. Only a
successful tmux command exit permits removal. The ledger's explicit requirement to retain failed kills took precedence
over the planner's proposal to accept any exited command: a failed command can leave an incompatible live server.
Consequently, a directory containing a dead socket whose kill command fails can remain on disk too. The regression test
first proves that the real tmux command executes and fails on its dead-socket fixture.

## Open questions and possible follow-ups

None requires a decision to ship this plan. A future macOS run could validate the strengthened environment test on its
actual platform. The two discarded replacement-test gaps remain if priorities later justify a product testing hook. No
one-off mutation check was run; the review checked the claimed regression discrimination by source inspection.

## The PRs

- [#1927](https://github.com/scode/farhelm/pull/1927/changes) — strengthens the supervisor and helm tests and preserves
  unconfirmed test cleanup for retry; one draft PR, never marked ready or merged by this executor.

## Checks run, reused and skipped

Runtime validation used the recorder, pinned nextest 0.9.148 and tmux 3.7c, four global slots and zero retries.

- Run: `cargo nextest run -p farhelm-supervisor -p farhelm-helm -p farhelm-teststate -E <focused-selection>` through the
  recorder, run `b21917e6-f0ca-432f-8793-6d376ae7d879`. It executed 100 tests: 99 passed and the deliberate-reconnect
  fixture failed as described above. No selected test reported a runtime skip. The failed record and traces are
  retained.
- Run: the exact corrected routing test through the recorder, run `7294fa32-de1f-4c69-ad6e-0e6f2ce0a0c8`, passed without
  a runtime skip. Reused the other 99 passes from the first run because subsequent source edits changed only that one
  caller's observation placement and comment. This does not relabel the first run as passing.
- Run: `cargo fmt --all -- --check`; targeted all-targets Clippy for supervisor, helm and teststate; isolated
  `python -B scripts/check-test-sleeps.py` (289 delays, zero missing rationale); targeted Markdown dprint. All passed.
  Final helm/teststate Clippy and the sleep check ran after the correction. Supervisor Clippy was reused because its
  covered source did not change. An overlapping Clippy launch was stopped and rerun serially; the interrupted invocation
  is not pass evidence.
- Run: nextest discovery resolved every changed fixture consumer to a real test. The private finishing helper passed
  format/type checks and its wrapped-index parsing test. The rebased feedback index matches every remaining finding
  exactly once.
- Reused across careful rebases: the focused runtime and source-review evidence. Intervening main changes were queue
  records, triage decisions, discard bookkeeping and accepted host/tooling/setup/managed-checkout specification limits.
  They changed no executable source dependency, and ordinary checkout replacement remains covered. Index conflicts
  preserved both plans' removals; the source diff was verified unchanged after each rebase.
- Unavailable: Linux-hosted macOS cross-compilation. The bundled SQLite C build reached Linux's C compiler, which
  rejected Apple's target flags. This supplies no macOS type or runtime verdict; the test was checked against macOS
  function signatures by inspection.
- Skipped: full Rust, browser, desktop, installer, provisioning, JavaScript and doctest suites. The diff changes the
  named test contracts, their helpers and test cleanup; focused execution and static checks cover those affected paths.
  No browser or shipped product behavior changed, and no executable doc example changed.

Private evidence and the failed run's disposition are recorded in `farhelm-plan-unit-test-oracle-fixes-log.md` beside
the checkouts. Private records of review routing and outcomes use session `42b567ab-df67-4565-8c76-301e6604c4ff`, also
referenced there. Native tools exposed no token usage or independently reported model attribution; these remain evidence
gaps. Some earlier launch events were not recorded at the time; a gap record preserves that omission without inventing
counters.

## The review gate's outcome

The required fresh-context reviewer was launched through the native tool with gpt-6.1-sol and high effort explicitly
selected. The tool returned its review but did not independently attest to the model that executed it. The reviewer
received the full plan, authoritative outcomes and test-authoring checklist. Its findings were accepted and corrected:
cancellation invalidated the initial close-based silence proof, failed tmux exits needed retention, wrapped index
entries needed complete removal, and the nested-runtime timeout needed parent-liveness context. The failed-command
fixture also now proves its real failed-exit premise. The reviewer accepted the caller's post-response signal starting a
connected observation window and two agreed discards; its narrow follow-up found no issue in the runtime correction. The
commit and PR wording cold read passed. Implementation and validation stayed with the executor.
