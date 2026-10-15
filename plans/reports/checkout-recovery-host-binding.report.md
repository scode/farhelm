## What this was about

An agent can ask Farhelm to create a session in a managed checkout. Once the user approves that request, the same
session can retry it to recover the approved attempt without another prompt. Another attached host could repeat the
original session ID, retry key and request and recover that attempt under the original approval. The helm knew which
host sent the request but did not include it in the retry identity.

The maintainer chose a small code fix: bind retries to the requesting host as well as the asking session, including both
agent create and clone. The stored retry key now hashes that complete scope with unambiguous component boundaries and
stays within the supervisor's length limit even with an unusually long session ID. The specification explicitly names
the host in the approved-recovery boundary.

## Things you should know

Retries started before this upgrade no longer match their earlier records and require approval again. This is the
upgrade effect the maintainer accepted. Retrying the same request from the same host and session still recovers its
approved attempt. A different host gets ordinary approval, or uses its own existing commands-without-asking setting.

No broader session-ownership authentication, supervisor protocol or database change was needed. The fix uses the
helm-known host of the live requesting connection, rather than a host claimed in request text. The changelog and
completed triage records are included, and the resolved feedback file and index entry are removed.

## Open questions and possible follow-ups

None identified.

## The PRs

- [#2086: bind checkout retries to the requesting host](https://github.com/scode/farhelm/pull/2086/changes) — one draft
  PR for host-bound retry keys and regression coverage.

## Checks run, reused and skipped

- Ran the two focused regressions through the pinned nextest recorder: 2 passed, run
  `a644e10e-cf25-4b5a-bc48-cfe9da0ae772`. The integration case waits for approval, records the attempt only after
  approval, recovers it from the same host, then repeats the request from a second live host and verifies a separate
  approval card and no dispatch after denial. The pure test covers host and session separation, component boundaries and
  bounded output. An initial recorder invocation refused an unsupported `--offline` option before running any test; the
  corrected invocation passed.
- Ran all 63 helm agent-request tests through the recorder: 63 passed, run `c85bc6f3-6509-4a28-a7f5-ee4c306b09d5`,
  selection `cargo nextest run -p farhelm-helm -E 'test(agent_requests)'`. Both runs used four global slots, zero
  retries and pinned tmux. Retained output had no runtime skip marker; manifests report successful completion without
  forced cleanup.
- Ran `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `python3 releasing/check-changelog.py format`, changed-Markdown
  `dprint check`, and the isolated-interpreter `python -B scripts/check-test-sleeps.py`; all passed. The delay checker
  inspected 301 delays and found zero missing reasons. Builds and tests used an owned container. Inspected added-line
  whitespace and made a separate documentation pass over every touched file.
- Reused source-review, Rust formatting, Clippy, sleep-check and runtime evidence from change `txmwnsww` at `cb072ac7`
  through final head `ec6d8729`. The later changes only add this PR's ledger link and rebase onto main `aea13eeb`. Read
  the entire incoming diff: deliberate-local-setup specification wording, triage records and queue reports, the
  trash-warning copy fix, and the scrollable-dialog review filter do not alter retry implementation or its contract. An
  adjacent feedback-index deletion conflicted and was resolved by preserving main's index and removing only this plan's
  item. The source patch is byte-for-byte identical after rebasing; final Markdown checks passed.
- Skipped broader workspace, browser, desktop and installer runtime checks: this change affects the helm's retry-key
  scope, with both call sites and approval/recovery behavior covered by the selected Rust tests. No browser interaction,
  installation behavior or migration changed.

## Review gate's outcome

The prescribed fresh-context gpt-6.1-sol review at high effort, through the external harness, found no actionable
issues. It covered the full diff, plan, triage decision, surrounding approval/reconciliation code and the full
test-authoring checklist. Commit and PR wording passed separate cold reads. No scope departure, unresolved finding or
maintainer decision remains.

### Landing

Landed as #2086, the second of the round's four plans, tested stacked with the other three on top of main (see
claude-trust-decimals for the shared runs).

#### Review

An independent fresh-context review found nothing blocking. The key that a managed-checkout retry is matched by now
includes the host the request arrived on: the registry entry of the live connection, never a host named in the request.
Both callers that build the key (create, which covers spawn and managed checkouts, and clone) are bound, and nothing
else rebuilds it. Host ids are never reused, so a removed host's attempts cannot be inherited. Old and new keys cannot
cross-match, so a retry pending across the upgrade fails toward approval, never toward a wrong match; the stored request
fingerprints are unchanged. The new regression test would fail without the fix.

#### Fixes made while landing

- The changelog fragment said "retry key", a name users never see; it now names `agent create` and `--idempotency-key`
  and says such a request is asked about like any other.
- SPEC_impl.md still said keys are scoped to the asking session; both places now say the session and its host, matching
  SPEC.md.
- The key function's comment now says what the upgrade does to an old retry (approved afresh, creating a new session or
  checkout instead of returning the earlier one) and that the host is the registry entry, which approvals follow across
  a destination change or an adopted identity.

#### Left as is

A retry pending across the upgrade creates a second session or checkout instead of returning the first; the plan
accepted this one-time effect, and the original stays visible for the user to delete. Not verified: whether such a
managed-checkout retry gets a fresh path or a conflict message; either tells the agent what to do.

#### Checks

See claude-trust-decimals: the helm package was part of the protocol, helm and supervisor run (2386 passed).
