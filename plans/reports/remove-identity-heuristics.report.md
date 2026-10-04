### What this was about

Claude sessions could acquire a Resume target by matching nearby conversation records to the time of their first input.
A wrong match could make Resume append to somebody else's conversation. The agreed rule is that new identities come only
from the agent's own explicit report.

The stack removes that scan and its re-verification. A new launch with no accepted report offers the existing
fresh-start fallback. Existing Resume offers are preserved, including Claude identities originally found by the scan.
Exact-file checks used to verify Codex, Grok, Pi and OMP reports remain in place; Goose's report path is unchanged.

### Things you should know

The no-report diagnostic remains a supervisor log warning. It fires once after 65 seconds from first confirmed input for
a hooked launch that still has no identity; it is not a new helm notification. Resume retains its identity, and a new
launch resets the diagnostic timer. Per-session report admission and reconciliation remain; the removed global
coordination existed for the scan.

The final PR migrates supervisor databases from schema 23 to 24, dropping only the scan's record locator, ambiguity flag
and persisted first-input timestamp. Conversation IDs, their provenance and ownership evidence survive. The associated
TODO entry is removed. The review item about unrestored scan fields after a failed restart (A6-C5) is also removed:
those fields are gone, and the remaining restore contract is documented precisely.

The neighboring report-wait/retry work landed while this plan ran. This stack was carefully rebased over it, preserving
claim waiting, sender attribution and hook retry budgets; targeted integration checks exercise the combination. Desktop
private-helm changes were also inspected and introduce no identity-path conflict. The only newer main change at final
inspection was queue bookkeeping.

### Open questions and possible follow-ups

None requires a decision for this plan. Neither named feedback item was a scan-only issue to discard: the Claude
report-claim timeout issue was fixed and removed by the neighboring landed plan; Codex draft-status recognition remains
separate and untouched. The pending launch-representation plan remains separate too.

As requested, execution stops after this report. No additional plan is claimed and queue monitoring does not restart.
This harness has no separate push-notification facility; delivery is reported in the conversation.

### The PRs

- [#1534: report-driven identity tests](https://github.com/scode/farhelm/pull/1534/changes) — shared fixtures and
  explicit reports replace scan setup in tests that consume an identity.
- [#1540: require reports for new identities](https://github.com/scode/farhelm/pull/1540/changes) — scanner deletion,
  preserved historical Resume, diagnostic timer, specifications and website documentation.
- [#1542: remove obsolete scan storage](https://github.com/scode/farhelm/pull/1542/changes) — schema 24, historical-data
  preservation tests and the two TODO removals.

All three remain drafts in one linear stack. None is marked ready or merged.

### Checks run, reused and skipped

Run on the completed stack, with pinned nextest/tmux and four slots, zero retries:

- The final integration selection passed 69/69: hook reports, Resume/restart, wrappers, rename and restart concurrency
  (run `1116dda0-b758-41a2-a509-48b26f0e9cfa`). This exercises the removed storage fields through real supervisor and
  tmux sessions.
- The final schema-23 migration test passed (run `d6ce7e21-4707-45e9-aa58-b02fb031f3fb`), covering reported, historical
  and ambiguous rows, retained session values, Resume targets and fresh/migrated schema parity. The store and
  historical-supervisor-upgrade selection had 101 passing tests and one fixture-only failure (run
  `7ee68cb9-f8ff-4481-a1f9-666ce20b618c`). That new test failed its exact index-SQL comparison because fixture
  indentation differed, after data/offer checks had passed. The failure is retained; the exact test passed after
  correction and again with the review's faithful historical data. The other 101 results remain applicable because
  subsequent changes were confined to that fixture and comments.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings` both passed, covering
  test seams and the shipped binary configuration.
- `cargo fmt --all -- --check`, dprint on changed Markdown, the changelog-format check and the test-delay checker
  passed; all 260 recognized delays have rationales. A separate documentation pass and final name search found obsolete
  fields only in historical migration code and fixtures.

Reused evidence, without treating overlapping selections as distinct coverage totals:

- PR2's 165-test selection passed (run `dc52a537-072c-4fe4-97ca-06df1d38e6b8`, recorded source fingerprint
  `5a38dc63afce` on base `8d9c74c9d770`). It covers all report-only vendors, record readers, identity state, reload,
  lifecycle and converted e2e consumers. Later field removal is covered by the store/integration checks above; preserved
  vendor verification code is unchanged. The eight focused acceptance/warning tests and four review-correction tests
  also passed (`328304ea-4018-4dd8-8a14-23f552e65a50`, `3802f45b-208f-42d2-a663-04e038d8c5b8`).
- The post-rebase 48-test selection passed (run `10bd551e-1efc-4f88-8915-f39f61c0fc5c`, source fingerprint
  `95a1fd6ff7b7` on base `1bf761b57a60`), covering the newly landed claim-wait behavior with hook/Resume/wrapper paths.
  Later schema changes have the final checks above, and main's subsequent queue-only change has no runtime interaction.
- PR1's nine identity-consumer tests and six converted fixture callers passed before the deletion
  (`77d34b61-51d7-4d32-bafe-6cc42d27f52b`, `5a3ac837-c056-4323-94c4-d83f91493625`). Their surviving behavior is covered
  again in the later selections.
- The website build passed during PR2 (28 pages). Subsequent page edits changed prose only, without changing
  frontmatter, routes, links or dependencies, so its build evidence remains applicable.

No selected runtime test reported a missing-substrate early return. A full workspace nextest run was considered and
skipped: the affected storage, admission, vendor-verification and lifecycle paths have focused coverage, and all-target
compilation catches removed-field callers. Browser, desktop-runtime, installer and provisioning suites were skipped
because this stack adds no UI, packaging or platform-specific behavior. No CI or release workflow was dispatched.

### Review gate's outcome

Each PR received the required fresh-context Opus high review (the CLI reported `claude-opus-5-5`). Implementation stayed
local. Findings were checked against the code and addressed: report-driven fixture lifecycle and assertions in PR1;
preserved identity replacement, byte-bound coverage, historical handoff and accurate scan-removal comments in PR2;
faithful historical migration data and precise storage/restore comments in PR3. No correctness finding remains open.

Optional fixture unification and a capture-state API rename/redesign were declined because they were not needed for the
agreed behavior. The final PR1 review also reassessed scope and found no unnecessary mechanism. Commit-message cold
reads passed consistency checks. The report receives its own fresh-context cold read before delivery.
