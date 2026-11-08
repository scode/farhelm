## What this was about

Adding a Tensorlake host could adopt a pre-existing sandbox merely because its generated name matched, then configure it
with this helm's credentials. Removing a host that had never requested a sandbox could also find and delete that
same-name resource. Recipe builds and recipe deletion had a related problem: cleanup could terminate another helm's
same-name builder and delete its snapshots. These collisions can arise from copied or restored state, or independently
matching short helm tags.

Triage asked Farhelm to use records it already keeps as evidence that it requested a sandbox. The maintainer required a
simple fix and explicitly preferred documenting unsupported shared state over adding a new ownership mechanism. The
third outcome was bookkeeping: preventing unrequested host adoption also closes the reported race with deletion under
**Sandboxes not on this helm**.

## Things you should know

Host setup now refuses a name match unless the host has a saved sandbox ID or outstanding create request. The refusal
names the sandbox and tells the user to delete the failed host entry first. That deletion leaves the existing sandbox
and its snapshots alone and makes it visible under **Sandboxes not on this helm**. A saved API key is still required for
deleting the entry. Existing saved-ID and outstanding-create recovery paths remain supported.

Builder cleanup first sweeps the saved sandbox ID. It recovers by name only for an outstanding create request, so a
different sandbox now occupying an old builder's name is left alone. Build starts remain progress records and do not
establish ownership; dropping them as evidence was an option the plan explicitly allowed and the simpler shape the
pre-code review accepted. No new ledger, lock, protocol field or vendor metadata was introduced.

Ordinary database durability is retained. An older installation or power loss that loses both the sandbox ID and pending
create marker can leave Farhelm's own sandbox unrecognized. The safe result is refusal and explicit cleanup. An occupied
build name can block every rebuild while that sandbox remains; the error names it and advises waiting or removing it in
Tensorlake. While the recipe exists, its build name remains hidden from **Sandboxes not on this helm**. Deleting a
recipe leaves an unrecorded same-name sandbox alone, but hosts using that recipe must be removed before the recipe can
be deleted. Direct cleanup in Tensorlake is available without removing those hosts.

The stack order is host fix, orphan-race bookkeeping, builder fix. This follows the plan's explicit decision that orphan
bookkeeping sits directly above the host fix, correcting the contradictory order in its outline. Each outcome still has
one draft PR, execution record and feedback removal. The specs describe the revised ownership and missing-record rules.
Resource monitoring remained active with manual heartbeat and pre-launch checks because asynchronous alert delivery was
not verified. No cloud calls or live-install changes were made.

## Open questions and possible follow-ups

Suspend and Resume still fall back to a host's sandbox name when its ID is absent. On a refused collision row, those
actions can suspend or resume the unrelated same-name sandbox. This reversible lifecycle behavior was identified during
the simplicity review and deferred outside the adoption/deletion outcomes. The changes here therefore do not protect
every action on a reserved name. No new TODO entry was added without authorization.

Running several helms from copied state remains unsupported. Existing records are evidence for recovery, not a new
provider-side ownership identity. Real Tensorlake behavior was not exercised; the tests use the shared in-memory model
to prove the selected ownership boundaries.

## The PRs

- [#3188](https://github.com/scode/farhelm/pull/3188/changes): refuse unrequested host adoption and skip name-based
  deletion without a create record; bookmark `plan/tensorlake-name-ownership/01-host-adopt-own-record`.
- [#3189](https://github.com/scode/farhelm/pull/3189/changes): close orphan-race feedback using that fix; bookmark
  `plan/tensorlake-name-ownership/02-orphan-race-closed`, directly above #3188.
- [#3190](https://github.com/scode/farhelm/pull/3190/changes): leave unrequested same-name builders alone; bookmark
  `plan/tensorlake-name-ownership/03-builder-adopt-own-record`, based on #3189.

All three are open drafts for the landing monitor.

## Checks run, reused and skipped

Host checks passed with pinned nextest 0.9.148, four slots, zero retries and verified pinned tmux. Recorder
`9b1e67c6-e9b4-4cee-8a97-b7ac8601dc38` ran 27 direct host lifecycle tests. Its Add selector matched nothing, so this
record supplies host coverage only. Recorder `11c2becd-1248-4b1c-9c00-e85a2595b6ec` then ran the 15 Add orchestration
tests using the verified `provisioning::service::tensorlake_add::tests` filter. Both passed with complete source,
reports, output and EOF, no omitted bytes and no runtime skips. Their common source identity was base `4a8bcf88` plus
dirty fingerprint `455cc7102c949837c1c214890afc3d3f735a5b444d273b09d523b2cca4a9508d`.

Both ran through `python3 scripts/record-test-run.py --runner nextest --kind development --tmux required` under the
pinned tools described above. The exact child commands were:

```sh
cargo nextest run -p farhelm-helm --lib -E 'test(/^provisioning::tensorlake_add_tests::/) | test(/^tensorlake::hosts::tests::/)'
cargo nextest run -p farhelm-helm --lib -E 'test(/^provisioning::service::tensorlake_add::tests::/)'
```

Those 42 disjoint passes cover collision refusal before configuration/admission, preservation of the foreign sandbox and
snapshots, zero API calls on refused-row deletion, leftover visibility afterwards, and legitimate recovery and cleanup.
They are reused across later comment/spec corrections, ledger URLs, bookkeeping and the builder change because the
tested host implementation and tests stayed unchanged. The same host reviewer verified that its requested final
correction changed documentation only.

An intermediate Add-selector attempt, recorder `a141b133-a3dc-4eaa-a4d8-15f6e0510288`, exited four with zero executed
tests because it omitted the module's `service` ancestor. Its complete evidence remains retained; it supplies no passing
coverage and is an authoring error, not a product failure or latent flake. The successful retry is a separate
observation.

Builder recorder `e96faaa4-6160-47db-b4b9-f9619ecd0735` passed all 31 selected cases, with 1552 selected out, through
`cargo nextest run -p farhelm-helm --lib -E 'test(/^tensorlake::build::tests::/)'`. It used the same pinned nextest,
four slots, zero retries and verified tmux. The report, source capture and output are complete, child and recorder exits
are zero, EOF was observed, no bytes were omitted, no runtime skips appeared, and cleanup has no limitation. This covers
fresh-build collision refusal, deletion of an interrupted-only recipe without API calls, and preservation of a different
name occupant after saved-ID cleanup, together with pending-create recovery, snapshots, cancellation and deletion. Its
source identity is base `3e455087` plus dirty fingerprint
`4c6c3342ad95872e493a4473a8798098d51061a788bedcd27754cfeac8967b53`. The later ledger URL and mechanical rebase change no
tested engine code or tests, so the pass is reused for the final draft. Across the three modules, 73 disjoint tests
actually passed; selected-out cases are not coverage.

Passed `cargo fmt --all -- --check` and `cargo clippy -p farhelm-helm --all-targets -- -D warnings` on the final host
and builder source. Passed `dprint check` on changed Markdown, the isolated `python -B scripts/check-test-sleeps.py`
source check (423 delays, zero missing reasons), `python3 releasing/check-changelog.py format` and
`python3 scripts/check-agents-md.py`. The orphan bookkeeping PR ran only Markdown formatting and instruction-reference
checks; it has no runtime change. The reference checker reports zero errors and three pre-existing citation warnings.
These checks are reused after the rebase because its only resolutions preserve both plans' feedback-index removals; that
index and the ledger were formatted again afterwards.

Main's changes between the host's original base `4a8bcf88` and `525a2ab8` were read in full: only a TODO decision, queue
state and another plan's report. The careful rebase had an empty host interdiff, so it required no new runtime checks.
The later rebase onto `34943a25` incorporated the settings-prompts plan: stronger cancellation of leftover Delete
questions, unconditional recipe-deletion wording, matching browser expectations, changelog fragments and bookkeeping.
All upstream diffs were read. The stronger question still calls the same backend deletion route; the warning remains
accurate for recorded resources. No helm backend, spec or ownership contract changed. Only the shared feedback index
conflicted, and both plans' removals were preserved bottom-up. Each PR's interdiff contains only that mechanical index
resolution, so no extra runtime validation was justified. The other plan's test results are not claimed here.

Skipped browser, desktop, installer and full Rust suites, live cloud checks and CI: the targeted engine/provisioning
tests and fake API cover the changed ownership decisions; UI, transport and installation behavior are unchanged. No
automatic CI verdict is expected on this repository.

## Review gate's outcome

Fresh Opus 5.5 at high effort accepted both proposed mechanisms before code was written. Each PR received its required
single fresh-context gpt-6.1-sol/high adversarial review. Host review found a documentation contract gap about legacy or
power-loss records; it was corrected in both specs and the teardown proof comments, and the same reviewer confirmed no
remaining findings. Orphan bookkeeping and builder reviews had no actionable findings. Separate cold readers checked all
commit/PR messages. The fresh native resume check reconciled the claim, goal, draft and retained test evidence after
compaction. No review finding was rejected; no simplicity fallback was needed.
