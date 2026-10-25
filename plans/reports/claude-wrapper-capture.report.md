## What this was about

Claude already worked through one wrapper that stayed running around it, but two or more such launchers prevented
Farhelm from learning which conversation Restart should resume. A new session therefore could not gain Restart from that
hook. The maintainer requested the same approach already used for Codex for consistency; no Claude incident prompted
this work.

The stack lets Claude report through wrappers at any depth. Farhelm follows the first reporting process, permits its
recorded parents to correct a child-first selection, and tells you what an earlier Restart would have resumed. A new
end-of-turn report lets the parent correct that selection without waiting for another start, resume, clear or compact.

## Things you should know

The accepted child-first gap remains: when the parent's report is lost, a nested Claude carrying a reporting hook can
report first, and Restart can resume that child's conversation until the parent reports and corrects it. Farhelm
remembers which process may supply Restart's conversation; that remembered authority is the process pin. Sessions
already running begin without one after upgrade, so their first report may also come from a nested child. They retain
their old command until relaunch and therefore gain end-of-turn correction only then. Existing Claude Restart offers
remain available.

Claude still reports a plain conversation ID without checking its saved transcript. Its single newest-report file
remains sufficient because both start and end-of-turn reports name the current conversation. A nested end-of-turn report
that the established parent excludes is acknowledged without a warning or any change to the saved conversation.

Installed Claude Code 2.1.310 was observed reporting the current ID on Stop before and after clear. Clear itself emitted
SessionStart, with no Stop observed during that transition. An actual background sub-agent completed through
SubagentStop with an agent marker; the surrounding Stop callbacks carried the parent's ID without that marker. These
observations cover one installed version and inherited permission settings, not every Claude mode or version.

The silent observation hook still coincided with Claude displaying its own “running Stop hook” line. The existing status
heuristic now recognizes only the observed prefix followed by an ASCII digit, under its existing composer and proximity
checks. Synthetic tests cover the two observed forms and near misses; captured agent screens were left unchanged. The
observation did not establish how much of the displayed duration the added hook caused.

The shared process pin preserves the existing database schema and serialized format; Codex's selection and confirmation
rules are unchanged. Documentation was updated on the Claude, Agent wrappers and Agent hook injection pages, together
with the specifications and one changelog fragment. The final PR removes the original TODO entry.

## Open questions and possible follow-ups

None required to complete this plan. Later Claude changes may require refreshing the observed callback and screen facts
through the existing real-agent observation procedure.

## The PRs

- [#2719](https://github.com/scode/farhelm/pull/2719/changes): share the existing process pin without changing Codex
  behavior.
- [#2725](https://github.com/scode/farhelm/pull/2725/changes): capture Claude through resident wrappers while preserving
  existing Restart offers.
- [#2727](https://github.com/scode/farhelm/pull/2727/changes): correct child-first capture at the parent's end of turn.

## Checks run, reused and skipped

All Rust execution used the recorder, nextest 0.9.148, four shared slots, zero retries and the repository-built tmux
3.7c with matching executable hash. Retained reports, source identities, output through each summary and cleanup records
were complete; no runtime `SKIPPED` return was observed. Selected-out cases are not claimed as coverage.

- PR1: `cargo nextest run -p farhelm-supervisor --lib -E 'test(codex_pin) | test(report_pin_replacement)'`, run
  `47be7052-d52b-454c-ae4f-a229518a33fd`, six passed. It covered the original PR1 tree on `afc9fb55`; removal of an
  unused re-export afterward did not change that behavior.
- PR2:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(procs::tests::claude_) | test(service::core::tests::claude_pin::) | test(codex_pin) | test(report_pin_) | test(report_survives_the_former_claim_deadline)'`,
  run `0a79d3a1-e10a-4966-a624-5b78dac93a17`, 22 passed. This covers attribution, admission, shared store persistence
  and capture-claim contention.
- PR2: `cargo nextest run -p farhelm --test e2e -E 'test(hook_identity::) | test(codex_identity::)'`, run
  `5eddc6cb-3230-4722-9ca3-235a51085071`, 27 tests plus fixture setup passed. Both PR2 runs covered the tree committed
  as `09bfb586`.
- PR3:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(service::core::tests::claude_pin::) | test(codex_pin) | test(report_pin_) | test(hook_argv_) | test(agent_kind::screen_reader::tests::claude_)'`,
  run `88cb5ad5-cb8e-49c2-a6eb-76575a833132`, 24 passed. This includes Stop selection, correction, silent refusals with
  a positive warning control, hook quoting and observed screen shapes.
- PR3: `cargo nextest run -p farhelm --bin farhelm -E 'test(hook::)'`, run `eca9e862-cad1-4abd-982a-34a01bc9376a`, 38
  passed, including two Goose hook tests also matched by the filter. It covers callback parsing, report-slot
  replacement, child markers and silence.
- PR3: `cargo nextest run -p farhelm --test e2e -E 'test(hook_identity::) | test(codex_identity::)'`, run
  `a4d2926d-b521-4f1b-9aa5-ba29fc090378`, 28 tests plus fixture setup passed. This observes the child selection before
  the parent's actual injected Stop, the correction notice once, actual Restart argv and the replacement's fresh Stop.
  All three PR3 runs covered `09bfb586` plus the final PR3 patch, with source fingerprint
  `022825fdff2fa050ccdd299a8b684bb25dfb9bf3ad4f4d9efcfc0adaad46b213`.

Final `cargo clippy -p farhelm-supervisor -p farhelm -p farhelm-fixtures --all-targets -- -D warnings` and
`cargo clippy -p farhelm --bins -- -D warnings` passed, covering test and shipped configurations.
`cargo fmt --all -- --check`, dprint on changed Markdown, changelog format validation and the source-only test-delay
checker passed; 323 delays had reasons. Formatting checks passed again after rebase.

The stack was carefully rebased onto `27f5a969`. Newer main contained an isolated create-request test-fixture fix,
bookkeeping and prospective plans/static mockups. No capture contract changed. All three per-PR interdiffs were empty
after rebase, and the upstream fixture and TODO edits were retained. The successful commands above are reused for those
unchanged patches; no extra runtime run was justified solely by the rebase.

The full Rust battery, browser, desktop, installer and provisioning suites were skipped because the focused checks
exercise the affected capture, hook and status boundaries. No web behavior, page routes or headings changed, so Markdown
checks covered the incidental docs edits without a website build or preview. No live installation was changed. The
installed-Claude observations above supplement the fake-agent tests; they are not a cross-version compatibility test.

## Review gate's outcome

Each PR passed its required single fresh gpt-6.1-sol reviewer at high effort, with no actionable findings. The first
PR's unused re-export cleanup also passed the same reviewer's follow-up. Commit and PR wording passed independent cold
reads. A separate own-model scope assessment accepted the small observed status-line exception within the existing
heuristic; no new status subsystem or broader capture rule was added.
