## What this was about

Pressing Stop while an agent was still starting could show a launch failure blaming the host's systemd user manager. The
maintainer chose “Exited — stopped before the agent finished starting” for this case, with the ordinary “stopped by
user” kept once startup had finished.

The change makes a recorded Stop take precedence over failures inferred from the stopped terminal, including an
unfinished checkout setup. It applies during normal observation, after a supervisor restart, and when Stop is retried
after an earlier attempt recorded its intent. A real failure report from the program that starts the agent still wins
while Stop is pending; a completed stopped exit keeps its first annotation. Restart and Replace share the changed stop
path.

## Things you should know

The initial outline's early state checks left a smaller race: Stop could be recorded while a failure observation was
waiting on files, before that observation committed. A separate scope review supported preserving direct-versus-inferred
evidence through the existing outcome write and temporary error cache, so both check the current Stop state. This adds
no session state or stored schema and does not change Stop's sequence.

Older pages show the new wording but use their ordinary exited icon in compact mode. Current pages use the stopped icon
for both annotations. The existing badge length cap can shorten the wording; the detail line wraps. These compatibility
choices were accepted in the plan. An unreadable or missing startup spec conservatively keeps “stopped by user”.

## Open questions and possible follow-ups

No product decision remains open. Remote hosts, terminal hosts, Restart and Replace share the corrected supervisor
logic; no separate reproduction for those operations was required by this plan.

## The PRs

- [#3210: report stops during agent startup](https://github.com/scode/farhelm/pull/3210/changes) — one draft fix PR
  based on main, head `8e36d007`.

## Checks run, reused and skipped

- Passed the final 10-case primary regression selection with `cargo nextest run -p farhelm-supervisor --lib`:
  pending-Stop classification, both reload branches, retried Stop, direct versus inferred failures, cached status,
  spec-read fallback and first-annotation retention. Run `ef8f6306-4c81-43cb-a230-a76cb41416bc`. The narrower
  four-fixture retry also passed as run `d7a99d9b-6b0f-4afb-8f87-1103b54bc9b4`.

- Passed the actual compact-row rendering test through recorded nextest: both Stop annotations use the stopped icon,
  with full accessible and hover wording. Run `8ce73bc5-be8a-4a4f-8679-1d29c2d037a2`.
- Passed 37 focused supervisor checks covering launch evidence, genuine failures, cached errors when writes are
  unavailable, cleanup, ordinary Stop and terminal-host exit handling. Run `d3b47bde-4898-4ac3-8a12-5bf6e2327fc6`.
- Passed all four recorded lifecycle cases: Stop during a real login shell's startup with and without a usable systemd
  manager, and genuine wrapper/login-shell failure controls. Run `e75cf4ab-dc7f-4f27-bcb8-9dd8389d1205`; no runtime
  substrate skip occurred. Its setup build is separate from the four test cases.
- Passed `cargo clippy -p farhelm-supervisor -p farhelm-proto -p farhelm-ui --all-targets -j 4 -- -D warnings` on
  `7a86f472`. The later change affects only the new unit-test fixture; product, UI and lifecycle code are unchanged.
  These runtime results remain applicable to those unchanged paths. The compact-row run covered the same UI/proto
  changes before supervisor integration.
- Passed `cargo clippy -p farhelm-supervisor --tests -j 4 -- -D warnings` and the final primary selection on `11b62255`,
  covering the corrected fixture. The final rebase over queue/report updates preserved the complete patch byte for byte,
  so these checks and the earlier unchanged-path results were reused without another runtime run.
- Passed `cargo fmt --all -- --check`, changed-file `dprint check`, `python3 releasing/check-changelog.py format`, and
  `<isolated-python> -B scripts/check-test-sleeps.py` (423 delays, none missing a rationale). The instruction-reference
  check passed with zero errors and one pre-existing triage citation warning.
- The first primary run (`91efa7d0-f3b7-4a55-b033-3030f6c14bf9`) passed six cases and failed four new fixtures: checkout
  acceptance had not been committed, and a planted scope could not be checked by the disabled manager. The first
  correction failed to compile (`1aa3c166-3382-4158-9fb9-796816e44318`) because its helper tried to clone a type that is
  not cloneable. The final fixture constructs valid scoped or unscoped evidence directly. Both failed runs are retained;
  neither is counted as a passing command. The intermediate test-target Clippy attempt failed on that same compile error
  before the final passing run.
- Browser runtime, full workspace tests, native desktop, installer and CI were skipped: deterministic supervisor and
  rendered-row coverage exercise the changed behavior; the browser edit is a comment, no geometry or browser logic
  changes, and no installer or desktop startup behavior changes. Remote hosts, Restart and Replace use the same
  supervisor paths, as the plan allowed.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no correctness, design or idiomaticity
findings in the combined patch. Its review was static; runtime evidence is reported separately above. The independent
commit-message cold read matched the problem, intended behavior and direct-failure exception, with no contradiction or
convention issue. The separate scope review supported only the commit-time and cached-evidence distinction described
above.
