## What this was about

This carries out all thirteen triaged outcomes about Farhelm's end-to-end tests. Some checks could accept broken
behavior or reject correct behavior, and failed fixture setup could leave processes running. The maintainer chose small
test fixes and a Linux-only suite because its process observations depend on `/proc`. Nothing changes for someone
running Farhelm.

The checks now require a bounded connection shutdown, fresh terminal output after a killed output sink (the subprocess
consuming terminal output) returns, the complete new environment value after Restart, and persistence that reaches at
least the activity value already observed. Fixture cleanup is armed before readiness checks. All thirteen feedback items
are removed and their execution records point to the same draft PR; no outcome was discarded or left pending.

## Things you should know

The original rc-file Restart test passed in its recorded baseline. That contradicts the prediction that it always timed
out, but its dependence on old screen content and its early parsing were still observable weaknesses. The new check
waits for the complete second value independently of the old visible screen, which Restart clears.

The first strengthened sink-recovery attempt failed because the test stopped consuming a flooding tab while waiting for
recovery. Farhelm correctly detached that stalled viewer. An exact reproduction confirmed this setup problem. The test
now consumes the flood during recovery and asks the shell for a new answer afterwards. The independent reviewer also
found that the initial flood check could match the shell's command echo; it now requires an actual rendered record. The
final focused run passed. These were same-session test corrections, with failed evidence retained; no product defect was
established.

Linux still exposes exactly the same 371 test names. Elsewhere, the binary contains one test that visibly reports why
execution is skipped. A temporary local cfg flip compiled and ran that skip body, then was restored byte-for-byte. This
checks the gated branch on a Linux compiler; it is not a macOS execution result.

The bounded container had no systemd user manager, so its systemd test returned early. A separate focused run against an
actual systemd 255 user manager passed without a runtime skip. The other changed tests also ran substantively.

## Open questions and possible follow-ups

None required. A one-off product mutation replacing the shutdown drain bound with an unconditional join was optional and
was not run. That mutation would deliberately make shutdown wait indefinitely for a peer that stopped reading. The
source review checked the buffer-size premise and timeout separation, and the focused test completed in about five
seconds with the independent stall timeout beyond its thirty-second deadline. A future mutation check could add direct
evidence that this particular regression is rejected.

## PRs

- [#1925: test: fix e2e oracles and failure cleanup](https://github.com/scode/farhelm/pull/1925/changes) — one draft PR,
  one change and bookmark; not marked ready or merged by this executor.

## Checks run, reused and skipped

Runtime commands used the recorder, pinned nextest 0.9.148 and pinned tmux, four global slots and zero retries.

- Before edits: exact rc-file Restart test, recorded repetition run `7ac55f7e-b710-41b2-8b6b-e5e433d5031f`, passed
  without a runtime skip.
- Focused eight-case selection, recorded development run `54221a8c-9b65-4ce8-be14-3ed0610a8038`: six substantive passes,
  one systemd runtime skip, and the sink setup failure described above. The six unchanged passing cases cover shutdown
  completing when a peer stops reading, Restart/environment parsing, activity persistence, identifying every process
  keeping a pipe open, Stop killing stubborn children, and a cheap request proceeding while a slow one is still in
  flight. Their evidence is reused because subsequent edits touched only the sink test and bookkeeping.
- Exact sink reproduction `8ebf5b8d-9304-4511-8093-1b976102b9f4` failed with the same stalled viewer; both failed runs
  remain retained. The consumption fix passed in `7e0edc04-b9ab-43aa-9e3c-f63a24aef19d`. After the rendered-record
  correction, final exact sink run `b832a676-1127-4b2c-bbab-d68f056939a9` passed without a runtime skip.
- Actual systemd 255 focused run `e9a08abe-bbeb-41e3-b8fc-4c6e7131df59` passed without a runtime skip.
- Linux inventories from `cargo nextest list -p farhelm --test e2e` matched all 371 names. Cfg-flip recorded run
  `c23157b9-5b0d-422e-b0d0-1b54b697fa82` compiled and emitted the explicit non-Linux skip reason; source restoration was
  verified.
- `cargo fmt --all -- --check`, `cargo clippy -p farhelm --all-targets -- -D warnings`, the isolated
  `python -B scripts/check-test-sleeps.py` (288 delays, zero unannotated), and dprint on changed Markdown passed.
- Bookkeeping verification confirmed thirteen Execution-only ledger edits and exact correspondence between the remaining
  feedback files and index entries.
- Careful rebases read the newly landed triage, planning and accepted host/tooling-limit clarifications, preserving
  their disjoint bookkeeping. Those clarifications concern hostile hosts, managed checkouts, setup/uninstall and one
  tooling run per checkout; they do not change the behavior these tests exercise. The index conflict was resolved by
  preserving both plans' removals. No executable source changed upstream, so existing runtime evidence remained
  applicable.
- Full Rust battery, browser, desktop, installer, JS and doctest runs were skipped: this diff changes only the named
  end-to-end tests and bookkeeping, and the focused evidence covers their affected behavior.

Private evidence is retained under the recorder IDs above. Its location and the two failed runs' disposition are
recorded in `farhelm-plan-e2e-oracle-and-cleanup-fixes-log.md` beside the checkouts.

## Review gate outcome

The required fresh-context gpt-6.1-sol high source review received the full acceptance criteria and test-authoring
checklist. Its only finding was the command-echo premise gap, accepted and corrected before the final sink pass. It
found no other correctness or unnecessary-complexity issue. A separate gpt-6.1-sol medium wording cold read accepted the
commit and PR wording. Implementation and validation stayed with the executor; no workhorse was delegated.

Private galaxy-brain evidence uses session `9317ea96-7a9d-4c8c-bdc5-c8d8cf7fc943`, referenced in the working log. The
native tools did not expose actual model attribution, reasoning effort or token usage, so those remain evidence gaps
rather than inferred measurements.
