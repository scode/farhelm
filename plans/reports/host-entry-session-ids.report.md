# Host-bound session identity

## What this was about

A session used to be identified across all hosts by the ID its supervisor reported. If a host was removed and another
claimed that ID, a waiting approval or a Delete/Replace confirmation could reach a different host entry. Create replies
could also collide with another host's session. The maintainer chose a durable identity made from a helm-assigned host
entry UUID, a colon, and the supervisor's local session ID.

The five draft PRs implement that identity across session routes, events, agent commands, viewer state and approvals.
Two hosts reporting the same local ID now have separate rows, counts and state. Removing and re-adding a host creates a
new identity, so old references cannot reach the replacement entry. Approval cards distinguish spaces, hidden formatting
characters and literal escape text in IDs, and Copy preserves the complete original reference.

## Things you should know

- The normal update path is unchanged. Existing sessions and supervisors from older releases keep working; their
  protocol still uses local session IDs.
- The helm database migration is one-way. It assigns host entry UUIDs and attributes existing viewer state through the
  cached host. Pins, seen state and notification marks without one attributable owner are dropped; an unattributable
  last selection is cleared. Removing and re-adding a host starts those convenience states fresh.
- Agent commands require full IDs from `farhelm agent sessions`. Old remembered bare targets are refused with that
  remedy. The exact caller's own `$FARHELM_SESSION_ID` remains accepted on its requesting entry. Full self-delete and
  self-prompt do not bypass the existing refusals; running-agent prompts remain disabled.
- An older browser tab must reload to use the new references. Bare IDs from stale tabs are refused rather than routed by
  guessing a host.
- Discovery JSON is schema version 7: the caller's session ID is full, and agent-facing host IDs are entry UUIDs. Caller
  markers also follow the entry/session pair.
- Pending approvals live only in the helm's memory. Restarting it for the upgrade drops those requests, so their cards
  do not survive reconnection. The waiting command may end without an approval result; issue a fresh request after
  reconnecting if the operation is still needed. Answering a new card still sends its random approval ID, not either
  session ID displayed on it.
- Execution found that the old 1,024-byte ingress range could produce full IDs too long for unchanged supervisors. The
  maintainer explicitly chose small UUID-sized limits: 40 bytes for the local ID and 77 for the full ID. Normal
  supervisor-minted UUIDs fit. Longer IDs from foreign or hand-edited state are omitted from usable listings.
- The display fix met its cheap gate by reusing identity rendering, full-width rows and the existing clipboard writer.
  No separate approval host validator or clipboard framework was introduced.

## Open questions and possible follow-ups

None requiring a maintainer decision. The work is open as a linear stack of drafts; landing belongs to the planning
monitor. The live installation was not changed.

## PRs

- [#2366: durable host entry identities](https://github.com/scode/farhelm/pull/2366/changes).
- [#2367: paired pins, read state and notification marks](https://github.com/scode/farhelm/pull/2367/changes).
- [#2368: public full session references and entry-bound commands](https://github.com/scode/farhelm/pull/2368/changes).
- [#2369: independent equal local IDs and removal of global collision refusal](https://github.com/scode/farhelm/pull/2369/changes).
- [#2370: distinguishable approval-card IDs and exact Copy](https://github.com/scode/farhelm/pull/2370/changes).

All three outcome Execution fields name their stable change IDs, bookmarks and PRs. Their feedback files and index
entries are removed. The unreleased breaking changelog fragment was updated in place.

## Checks run, reused and skipped

### Final stack

- `cargo fmt --all -- --check`, both required Clippy commands (`cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings`), targeted `dprint check`, changelog format and instruction-pointer
  checks passed. The isolated test-sleep checker inspected 304 delays with no missing rationale. Clippy initially found
  mechanical closure/borrow/condition and test-error extraction lints; those were corrected in their original PRs and
  verified by the same reviewers before the final successful commands.
- `cargo build` and the Dioxus 0.7.10 release web build passed. The first unqualified `dx` invocation selected another
  installed tool and failed before building; invoking the existing Dioxus executable resolved that command collision.
- `cargo nextest run -p farhelm --test e2e` filtered to `agent_listing_real_stack`, `github_checkouts`,
  `prompt_running_agent`, `change_hints` and `merged_hosts`: 12 tests and the fixture-build setup passed, with 379
  excluded/ignored cases. Run `d93fece9-c78a-4918-acf6-fecd9ec7788c` captured clean tip
  `c5f89a51f3d0f0acf5475ed283008a60337daccd`, source fingerprint
  `930452dda2688c86a2edac736485f14351231b3100cfc7d6c1312a4647288117`. The report and output are complete, with EOF, zero
  omissions and no runtime `SKIPPED` marker or cleanup limit. Covers shipped CLI discovery, lifecycle, create, clone,
  actual checkout hooks/recovery, cross-host operations and the disabled-prompt refusal.
- `npx playwright test` selected `agent-relay`, `approval-layout`, `session-confirmations`, `delete-precondition`,
  `replace`, `terminal`, `terminal-tabs`, `terminal-files` and `notifications` on Chromium and WebKit. Run
  `dfc0c87c-7e3e-4728-b233-90479b282b1d` executed 320 cases: 318 passed and the same session-switching test failed on
  both engines. Its mounted socket was correct; an outdated assertion looked for a raw colon rather than the URL's
  encoded `%3A`. The assertion now compares the complete encoded session pathname. A targeted search found and corrected
  the same assertion in credential-rotation recovery. Run `e8137ccc-04fe-4173-b549-c384f5793da4`, using
  `npx playwright test tests/terminal.spec.ts tests/auth.spec.ts -g` with those two title phrases, passed all four cases
  (two per engine). Source fingerprint `e8e89d632ad0d633241a49d0104dffc80f59fb53d7851be4f608d23a952920e4` identifies the
  corrected working tree above tip `c5f89a51`. Both reports and outputs are complete, with EOF, no omitted bytes, no
  skips, flaky outcomes, unrun cases or cleanup limit. The same reviewer verified both corrections and reuse of the
  other 318 passes: only test assertions and their comments changed. The original command remains a failed observation.
  One intervening attempt (`aedd8e01-63b4-4c88-ad6d-c00c14674d84`) selected no tests because of an over-anchored title
  filter; it is retained and supplies no coverage. Product builds were reused for the assertion-only retry.

### Earlier PR evidence and reuse

Every runtime command ran through the recorder with four nextest slots and zero retries, or one browser worker and zero
retries. Unit selections used no tmux; final real-stack and browser selections required and verified the pinned tmux
executable. The retained run IDs identify exact filters and source fingerprints; coverage below is deliberately limited
to the named selections.

| Command and selection                                                                                                                 | Retained run and result                                                                                                                                   | Reuse at the final stack                                                                                                                                                                                                                                                            |
| ------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo nextest run -p farhelm-proto --lib -E 'test(session_ref::)'`                                                                   | `b9717d26-4e11-46d9-b54d-12c1e870435d`: 4 passed                                                                                                          | Parser behavior unchanged; later re-exports compiled in all dependent targets.                                                                                                                                                                                                      |
| Helm/proto library store and parser selection                                                                                         | `10f6296e-2a61-4108-9892-afc937976b79`: 172 passed, 2 historical migration fixture failures; exact retry `dfea0aaa-b48e-4c95-99bd-594dae7647d7`: 2 passed | Original failure retained. Only synthetic migration premises changed; later store module run covers final schema.                                                                                                                                                                   |
| Helm library paired-state/store/aggregate and affected operation selection                                                            | `6dec0119-cb2b-4812-aeb1-b1e9e9f2cbf1`: 207 passed                                                                                                        | Foundation observation; final retirement and browser selections cover later changes to these paths.                                                                                                                                                                                 |
| Helm/UI library full-reference, stale-entry, transport, migration and API-error regressions                                           | `51b78a3c-ffe0-4876-827f-5fbb037efbad`: 23 passed                                                                                                         | Boundary contracts unchanged by cache retirement, card display or mechanical lint fixes.                                                                                                                                                                                            |
| Helm/UI affected modules and API contracts                                                                                            | `d4bd0ad1-566c-4008-99a5-8b09822944b3`: 670 passed, 4 stale test expectations failed; exact retry `9fdef828-cebe-410f-8161-cb07d2596484`: 4 passed        | Failure retained. Corrected only expected full IDs and entry-local not-found behavior, preserving refusal, pending-card and no-dispatch assertions. The reviewer verified the correction and unchanged successful coverage. This is not described as a successful original command. |
| `cargo nextest run -p farhelm --bin farhelm --test agent_cli --test spawn_cli` with the recorded binary/renderer/instructions filter  | `cfdc4734-c8d6-4fb9-8c6c-b7dbe24b0157`: 81 passed                                                                                                         | Schema 7, full IDs and self-restart discovery unchanged; the later condition collapse preserves short-circuiting.                                                                                                                                                                   |
| Helm library schema 63/64/65, equal-ID, create-withdrawal, approval and notification regressions                                      | `357f839d-cb30-4b5c-9500-d29200be716c`: 11 passed                                                                                                         | Tested corrected retirement source fingerprint `8cd17ed5bf7d94f7960cc7ca563d60ce6c9f3fbc0331d2c99f1014073b5f2b01`; later changes are card-only and mechanical lints.                                                                                                                |
| `cargo nextest run -p farhelm-helm --lib` filtered to store, manager, aggregate, events, sessions, agent requests, approvals and ntfy | `caf2b8f7-d7f6-46c8-a9c3-b6beb2201d85`: 559 passed                                                                                                        | Same corrected retirement fingerprint as the focused run; later schema-62 test error extraction keeps the same oracle.                                                                                                                                                              |
| `cargo nextest run -p farhelm-ui --lib --features desktop -E 'test(approvals::) \| test(peer::)'`                                     | `77513bbe-3f6f-451d-b2a3-10a3a0904582`: 24 passed                                                                                                         | Card source fingerprint `826e39e9968bcb4be10a82449eaffea6112451d5031cd3e49ad82b994eaba696`; only unrelated lint and ledger changes followed. Includes the production Deny handler sending the approval ID.                                                                          |

Two retirement compilation attempts (`29bb2160-f765-437d-89e1-233abd5b94fa` and `d405ff83-8bce-4250-a75f-f67e3562d88f`)
failed before any test ran because new test imports were missing. Both are retained; neither supplies runtime coverage.
The second attempt's source identity is uncertain because formatter completion had not been collected. The successful
retry froze source after formatting. These and the test expectation failures were authoring observations fixed in this
run, not latent flakes.

Create-withdrawal coverage deliberately separates two connected REST cases from two actual-transport-withdrawal cases
that invoke the existing acceptance helper after a delivered reply. The latter prove stale cache seeding is refused
while the accepted reply retains its original entry prefix; they do not claim to force an HTTP scheduler race.

The full workspace and browser batteries, release/installer/provisioning gates, real vendor agent turns and desktop
window smoke were skipped: targeted affected behavior is covered above; the change does not alter those independent
systems. UI library tests built with the desktop feature and both browser engines cover the shared approval rendering
and answer-handler seams. They do not validate a running native desktop window or its integration with the operating
system. Disabled prompt delivery remains intentionally unexecuted, with its end-to-end refusal checked instead. No CI
run is implied by draft creation; repository CI runs only on demand.

### Rebase

The stack was carefully rebased onto main containing the separately landed launch-directory enforcement and trailing
review bookkeeping. Their complete diffs were read. The launch/tab helper changes and internal tab-shell CLI arm do not
change session identity or the supervisor protocol; shared spec passages are separate. Every per-PR interdiff was empty
after that rebase, and no conflict resolution was needed. Subsequent lint-only and browser-oracle amendments were
reviewed separately. A final rebase onto `4b88da40` included only TODO, triage and planning bookkeeping changes. Its
shared-ledger conflicts were resolved by retaining upstream records and each PR's own execution fields; no contracts,
product code or tests changed. The final real-stack and browser evidence therefore remains applicable. New planning
records added after that base do not change this stack's behavior.

## Review gate outcome

Each PR passed its single fresh-context gpt-6.1-sol/high general reviewer, with the full goal, Decisions, complete
outcome entries and test-authoring checklist. The same reviewers verified corrections; no second general reviewer or
swarm was used. Commit/PR wording passed independent cold reads. The combined boundary cut passed the required scope
reassessment, and the approval display complexity gate was accepted for the reuse described above.

### Landing

Landed as #2366, #2367, #2368, #2369 and #2370, merged bottom-up, with test-only fixes made during the landing (below);
no product code changed at landing. It was the first of four plans in this landing round, with helm-lock-outlives-work,
hosts-panel-menu-close and template-list-current-labels after it.

#### Release relevance

This is the release's largest change and is marked breaking. The report covers it; in short:

- The helm's database migration is one-way, and some viewer conveniences that cannot be attributed to one host entry
  (pins, seen state, notification marks, the last selection) are dropped.
- Agent commands need the full session IDs `farhelm agent sessions` prints; remembered bare IDs are refused.
- Discovery JSON moves to schema version 7.
- Approval requests waiting when the helm restarts for the upgrade are lost; the agent has to ask again.
- An open browser tab must be reloaded after the upgrade.
- Session IDs are limited to 40 bytes from the supervisor (77 for the full ID); longer ones, which only foreign or
  hand-edited state could produce, are left out of listings.
- Supervisors are unchanged and older ones keep working.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained archive-recovery-reproduction (a guard on a deleted checkout's recorded trash
location, in the supervisor), summary-card-observed-yolo (the session hover card mentions observed YOLO), and
documentation and plan bookkeeping. A fresh-context reviewer and I read this plan against those and against the three
plans landing after it:

- The hover card change reads only a session's launch and its observed-YOLO flag, never its ID, and approval cards are
  drawn outside the session rows it changed, so it does not interact.
- The trash-location guard is entirely inside the supervisor, which still uses bare local IDs.
- helm-lock-outlives-work changes when the helm releases its state-folder lock; this plan's database migration still
  runs while the lock is held, before anything else opens the database.
- hosts-panel-menu-close and template-list-current-labels touch neither session identity nor host entries.

The only conflicts were in the review feedback queue's index, where each plan removes its own entries; all removals are
kept. While the landing ran, main also gained three planning changes that edit TODO.md and the triage record, not code.
The landed code is identical to what the landing's checks below tested.

#### Fixes made during the landing

The executor ran 9 of the roughly 40 browser files it edited (counted from the diff), and only a selection of Rust
tests. The landing's whole-suite runs found tests that still expected the old bare IDs, or a session to keep its
reference after its host was removed and re-added (the plan's own design gives it a new one). Each fix sits in the PR
that introduced the change it follows; none changes product code:

- In #2366, #2367 and #2369, the helm's checkout-configuration migration tests rebuild an older database to check the
  upgrade path, and that rebuild now also undoes this plan's schema changes: it drops the host entry ID column, restores
  the seen-state table to its old shape, and recreates the one-owner index on cached sessions that the plan removed.
- In #2368:
  - Two helm host tests now compare session rows with full references.
  - A new browser-test helper gives the supervisor's own part of a full ID. The managed-checkout and clone browser tests
    use it to wait for a launch marker the supervisor prints, and the file-attachment tests use it to check the folder
    uploads land in.
  - The test that removes and re-adds a host now checks that the session survives with the same supervisor ID but a new
    reference, and its cleanup finds the session again under the new entry. Before that change, it left a session
    running on the test's remote host, which failed later tests.
  - One session-list test built a page selector from a session ID, and the colon in a full ID broke the selector.

#### Possible follow-ups

- hosts-panel-menu-close, landing after this plan, closes an open session menu whenever the hosts panel changes height.
  Browser tests that open a session row's menu without waiting for host rows to settle could become flaky. Nothing
  failed for this reason in these runs.

#### Checks

Run now, on the four plans stacked together on main:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed file, and `python3 releasing/check-changelog.py format`: all passed,
  and clippy again after the fixes.
- The whole Rust workspace through the test recorder with pinned nextest and tmux, four slots and no retries
  (`cargo nextest run --workspace --exclude farhelm-desktop`): 3934 passed and 4 failed in run
  `7067248e-06f2-4bd7-90ff-63250d8fff42`, the four helm tests fixed above. After the fixes, those two test modules
  passed in run `19fa2fc6-b6f3-442a-8682-5e3614f48040`, and the whole helm library (1232 tests) in run
  `cbf11585-1556-4055-ba01-7041b35a4f28`. The UI library built with desktop support: 714 passed, run
  `1560df43-83f8-4586-87cf-c71d56f481fc`. `node --test` in the UI's JavaScript test directory: 311 passed, run
  `5f9b15b4-5549-46ed-8f7a-3528415256fa`.
- The full browser suite (`npx playwright test`) on Chromium and WebKit, one worker, no retries: 2025 passed, 27 failed
  and 76 did not run, in run `8214f916-6dda-4c7a-9d91-35ed24e150e8`; all failures were the browser tests fixed above,
  and every test that did not run was in the multi-host file. After the fixes, the clone, managed-checkout,
  file-attachment, session-list and multi-host files ran again: 417 passed and 3 multi-host tests failed, in run
  `796a5815-5f13-4ded-8d5c-a796487bd993`; those three came from the remove-and-re-add test's leftover session, and after
  its cleanup fix the multi-host file alone passed all 116 tests, run `c5409925-1c07-4c7f-9e5c-065728a7e123`. The
  remaining skips are by design: the real-agent tests, WebKit's clipboard limits, and one visual capture.
- Skipped: the installer and provisioning against a CentOS host (no change to installing Farhelm or setting up hosts),
  the native Mac app (not runnable here), and doctests (no documentation examples changed).

One thing in the executor's report became untrue at landing: it skipped the full Rust and browser suites on the ground
that its selected checks covered the affected behavior, and the full suites then found the stale tests above. The
product behavior the report describes held.
