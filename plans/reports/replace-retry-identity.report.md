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
