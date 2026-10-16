## What this was about

After losing a helm's database and recovering with a fresh one, a different host could receive the same numeric host ID
as an earlier host. If it knew that earlier host's session ID, retry key and exact request, it could reuse the earlier
approval to finish creating a session in a managed checkout. It could not choose different launch contents.

Each helm database now keeps its own durable random namespace. Agent create and clone retry keys include that namespace,
the asking host entry and the asking session, so independently created databases cannot reuse those retry records.
Retries within the same database remain stable, and approvals still follow a host entry across adoption.

## Things you should know

The maintainer chose to give existing databases a namespace too, avoiding a permanent compatibility branch. A keyed
retry already in flight across this upgrade needs approval again and, if approved, creates a new session or checkout
instead of returning the earlier one. The changelog fragment states that consequence.

The helm creates the namespace when creating or migrating its database. Ordinary reopening preserves it. Offline
commands can still open older schemas without migrating them, because only keyed agent requests read the namespace. No
supervisor or wire-protocol change was needed, and no broader session-ownership mechanism was introduced.

The triaged feedback item was removed and its execution entry names this PR. Nothing was left outside the agreed scope,
and no decision is waiting on the maintainer.

## Open questions and possible follow-ups

The full sequential recovery scenario with a malicious supervisor and a surviving target reservation was not exercised
end to end. The regression instead establishes identical host IDs in two real databases, distinct durable namespaces and
different resulting retry keys; existing create and clone tests inspect the keys actually sent to the target.

## The PRs

- [#2172 — fix: scope agent retries to each helm database](https://github.com/scode/farhelm/pull/2172/changes), draft,
  based on main, head `e8f0a49636e45e387056aee2fb7454a80cf008a4`.

## Checks run, reused and skipped

- Ran
  `python3 scripts/record-test-run.py --runner nextest --kind development --tmux required --selection 'helm agent requests and store migration tests' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-helm --lib -E 'test(agent_requests::) | test(store::)'`.
  Recorder `d8c37b7b-9065-45f9-bde8-0a082a40f7a1`: 222 passed, zero failures or flaky outcomes, 896 selected out. The
  JUnit report is complete, retained output reached EOF without truncation, and no runtime `SKIPPED` marker was present.
  The required pinned tmux and nextest were prepared before the run. This covers both affected modules, including
  fresh/migrated schema parity, namespace durability, genuine older-schema offline opening, and agent retry behavior. It
  is not a whole-workspace test pass.
- Ran `cargo clippy --all-targets -- -D warnings` and `cargo fmt --all -- --check`; both passed.
- Ran the test-sleep checker with its isolated pinned interpreter: `python -B scripts/check-test-sleeps.py` inspected
  302 delays, with zero missing reasons. Changelog format, dprint on the changed Markdown, and whitespace checks passed.
  Adding the PR link required one Markdown wrapping correction, after which dprint passed.
- Reused the successful Rust tests and source review for the final head: subsequent changes only formatted Markdown and
  added the PR link to the execution ledger. Executable Rust and tests are unchanged. Main's intervening changes were
  confined to unrelated plan state and reports, so no rebase or repeated runtime validation was warranted.
- Skipped browser, desktop, installer and full-workspace runtime suites: this changes helm persistence and opaque retry
  keys, with no UI, supervisor or wire change; the selected tests cover the changed integration points and migration.

## Review gate's outcome

The required fresh-context gpt-6.1-sol/high source review completed through the prescribed shell-out path. Its complete
artifact reported no actionable findings after reviewing correctness, scope, design, idiomatic code, upgrade behavior
and test premises. It ran no runtime tests; the recorded validation above belongs to the executor.

A separate fresh native gpt-6.1-sol/medium cold reader understood the commit and PR wording, found no contradiction with
the diff or convention violation, and the executor accepted it. No corrective source-review round or scope departure was
needed. External per-turn usage evidence is retained privately; native and main-session usage counters were unavailable,
and the external JSON did not echo an actual model identity beyond the explicit CLI selection.
