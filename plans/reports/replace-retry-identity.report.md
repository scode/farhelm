# Replace retries across controls and after a refused create

## What this was about

Replace had two retry problems. If creating the replacement succeeded but removing the original failed, retrying from
the other control could create another replacement. If creation was refused because the source's folder was missing,
restoring the folder and trying again kept replaying the old refusal.

The maintainer chose one shared retry identity for the sidebar, the top of the open session and the interrupted-session
card, plus a fresh attempt after a confirmed creation refusal. The first outcome was restricted to existing UI state;
the second could add a reply field but could not change durable replay or identity storage.

## Things you should know

Both outcomes stayed within those gates. The controls now share one source-tagged identity for unresolved plain Replace.
It survives navigation within the page, so retrying through another control recovers the replacement already created.
Success, definite refusal, cancellation of that same intent, Replace with, or replacing another source ends it. A late
response or cancellation can clear only the source and identity it belongs to, including when the user switches from one
source to another and back. SPEC.md now states the broader same-operation principle and the Replace identity rule.

For ordinary keyed creation, the supervisor now proves a settled refusal only when the exact request and key lifetime
match, the stored outcome is failed, and no session row survives. The proof recognizes the digest the store keeps after
reopening, as well as the original request fingerprint. Retained and uncertain launches do not qualify; managed-checkout
and agent-request recovery encodings are excluded. The helm accepts the proof only on a create reply, never on a failed
delete after creation succeeded, and passes it through the existing browser marker. Durable keyed replay is unchanged.

This changes the supervisor protocol to 53, so remote hosts need Update to connect. Both PRs include fixed changelog
fragments, remove their feedback entries, and update their triage execution records. The live installation was
untouched.

## Open questions and possible follow-ups

The top-of-session control and interrupted-session card still keep separate confirmations: a question opened in one
surface is not the other surface's question. Sidebar and header Replace also retain different coordination with other
session actions, so this work does not make all their disabled controls identical. This run did not establish an
additional user-visible failure caused by those differences. The plan explicitly left them for follow-up under the new
same-operation principle. Retry identity is page state; this work adds no durable browser storage.

## The PRs

- [#2205: share Replace retries across controls](https://github.com/scode/farhelm/pull/2205/changes), draft, based on
  main.
- [#2206: retry Replace after a settled refusal](https://github.com/scode/farhelm/pull/2206/changes), draft, based on
  #2205.

## Checks run, reused and skipped

- UI: `cargo nextest run -p farhelm-ui --lib -E 'test(replace) | test(row_op)'` passed 16 selected tests in
  `e801c9b2-5a04-49e3-b7c5-d308251f3b00`; 497 filter exclusions are not runtime coverage.
- Backend: the focused `cargo nextest run -p farhelm-proto -p farhelm-supervisor -p farhelm-helm --lib` selection passed
  eight tests in `e5713b97-70af-4a3c-9bd8-3147999d3a1b`: proof encoding, protocol version, failure replay, exact-request
  proof, pending/created/ambiguous exclusions, and successful creation followed by a delete incorrectly claiming
  non-acceptance. After the review fix, the four supervisor cases passed again in
  `226f7383-8d6a-4b54-b825-7b4df9c70455`, including actual store reopening and digest-form replay. The two runs excluded
  2413 and 1137 tests respectively. All nextest commands used the recorder, pinned nextest 0.9.148 and pinned tmux, four
  slots and zero retries. Selected output had no runtime `SKIPPED` marker.
- Browser:
  `replace.spec.ts -g 'starts fresh after repairing a missing folder|retries an unknown outcome|shares a committed retry'`
  passed 12 cases, six each on Chromium and WebKit, in `3b5cb0b1-7e58-4927-afce-66e7e204e2ff`. These cover real
  missing-folder refusal and repair from both controls, unchanged keys after unknown outcomes, and a single replacement
  after committed cross-control retries. The recorder verified complete reports and cleanup, one worker, zero retries,
  and no skips. Native and Dioxus web builds passed before this run.
- Earlier cross-control browser run `badb785a-fb3b-4d35-bfa9-c0cdc7cdc591` passed four cases on both engines. All four
  older-question cancellation cases also passed in `49a19110-385f-4f76-bd4f-5b8fcd347b50`; that run had seven passes and
  one navigation failure, so it is not a clean overall pass. The navigation fixture was corrected and the affected
  cross-control cases passed in the later runs above.
- Three same-session browser failures are retained privately: `d404df5c-a5b5-4106-891b-4a22ddf312d5` expected the wrong
  HTTP status for partial success; `49a19110-385f-4f76-bd4f-5b8fcd347b50` exposed the navigation fixture's menu
  dismissal/focus assumptions; `fe365719-1b61-4fda-9591-135c4590e4c0` used the wrong import for its readiness helper.
  Later passes do not erase these failures. These were corrected during this plan, not classified as latent flakes.
- Final all-target Clippy, Rust formatting, selective dprint, changelog format, agent-instruction check and test-sleep
  checker passed; the latter inspected 302 delays with no missing rationale. A separate documentation pass covered every
  touched file. Existing Error constructors received only false defaults and unrelated patterns retained their
  assertions.
- Reused after the clean rebase onto `af69dba8`: the tests, builds and reviews above. The full intervening diff changed
  template-name display and its tests, plus bookkeeping; it shared no Replace functions or contracts. Both PR interdiffs
  were empty. The only later edit was the second PR's ledger URL. The first outcome's UI and cancellation evidence also
  remains applicable because the second outcome changes no UI logic.
- Broader Rust/browser suites, desktop-specific checks, installer/provisioning, website builds, doctests and hosted CI
  were skipped: the focused cases cover the changed retry, refusal and provenance boundaries; no desktop-specific,
  installer, site or executable-documentation behavior changed. Creating draft PRs does not itself require a wider run.

## Review gate's outcome

Each outcome passed its required fresh-context gpt-6.1-sol/high shellout source gate after one correction. The first
review caught stale confirmation cancellation and a live-fixture readiness assumption. The second caught the missing
digest-form refusal proof after store reopening. The executor fixed them and the same reviewers rechecked the revised
diffs with no actionable findings remaining. Source review was static; runtime evidence is reported separately above.
Both commit/PR messages passed Sol/medium cold reads. Implementation stayed local under no-workhorse mode. Raw CLI usage
is recorded privately; actual served model attribution and native/orchestrator usage are unavailable. This report also
receives the separate native clarity and public-hygiene cold read before delivery. The executor leaves both PRs draft
and does not merge them.

### Landing

Landed on its own in round 81, as #2205 and #2206, merged bottom-up and unchanged from what the report describes.

#### What landed since the stack was based, and whether it interacted

Only plan-queue bookkeeping reached main between this stack's base and its landing, so the landing rebase was clean and
both PRs' diffs are identical to the ones the executor delivered. The base already held the work this stack sits next
to: agents deleting sessions and initial prompts on session creation (#2192, #2200), initial prompts in the launcher,
including the landing fix that makes Replace with send its prompt (#2203), agent retries scoped to each helm database
(#2172), the cancelled-download fix (#2197) and skipping Claude's lock waits at startup (#2201). The landing monitor
read the stack against them, and a fresh-context reviewer did the same independently. Neither found an interaction:

- The supervisor's new proof that a create was refused and kept no session covers ordinary, named and prompt-bearing
  creates, including Replace with a prompt. It excludes agent requests and managed-checkout creates.
- Deleting a session marks its create record as created, never as failed, so a deleted session can never produce the
  proof. The helm accepts the proof only on a create reply, and a test pins that it ignores one on a failed delete.
- Retry keys for agent requests, the helm's tracking of pending requests and startup trust cleanup are untouched.

#### Upgrade

#2206 raises the protocol from 52 to 53 by adding an optional field to the existing error reply. Protocol 52, from the
agents-delete and initial-prompt work, has not shipped either (v0.29.0 carries protocol 51), so the next release already
requires remote hosts to be updated, and this adds no new step for users. No stored data changes. The changelog entry,
the commit message and the report all say remote hosts need Update; release curation can merge that line with the
near-identical one in the initial-prompt and session-delete entries. Both defects date from #1936, which shipped in
v0.28.0, so each PR keeps its own "fixed" changelog entry rather than amending an unreleased one.

#### Fixes made during the landing

None.

#### Possible follow-ups

- The session launcher itself, for a new session or a Replace with, still keeps its retry key after a refusal when the
  form is unchanged, and ignores the new proof. So restoring a missing folder and submitting the same form again still
  replays the old refusal there. This predates the stack and was outside its scope.
- The report already lists that the top-of-session control and the interrupted-session card keep separate confirmations.
- Minor: the code comment describing the "definitely not accepted" reply header lists only local failures and checkout
  conflicts; it now has a third source.

#### Checks

- Run now: none. Main gained only plan-queue bookkeeping since the base.
- Reused, from the executor's runs: the UI Replace tests `e801c9b2-5a04-49e3-b7c5-d308251f3b00` (16 passed), the
  protocol, supervisor and helm selection `e5713b97-70af-4a3c-9bd8-3147999d3a1b` and the post-review rerun of its four
  supervisor cases `226f7383-8d6a-4b54-b825-7b4df9c70455`, and the browser Replace cases on Chromium and WebKit
  `3b5cb0b1-7e58-4927-afce-66e7e204e2ff` (12 passed), plus its builds and static checks. These were recorded on an
  earlier base; the executor reused them after a clean rebase over the template-name escape fix (#2204), which shares no
  code with Replace, and the landing rebase added only plan bookkeeping on top. The report lists its retained failures
  separately; they are not counted as passes.
- Skipped: everything the report skipped, for the reasons it gives.

Nothing in the executor's report became untrue at landing.
