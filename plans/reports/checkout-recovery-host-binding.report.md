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
