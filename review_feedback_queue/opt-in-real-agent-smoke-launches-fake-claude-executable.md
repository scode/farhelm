# The opt-in real-agent smoke launches the fake Claude executable

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The opted-in real-agent smoke still launches the fake executable.

## Details

F230 — **definite** — `e2e/tests/real-agent.spec.ts:477` — The opt-in real-agent smoke launches the fake Claude
executable

The ordinary stack installs and prioritizes a fake Claude regardless of the real-agent opt-in. The smoke requests that
bare command but waits for real-vendor onboarding and reply output, so its configured invocation cannot validate the
intended integration. Supply an explicit real executable and deliberate credential environment for opted-in runs,
preserving fixture isolation for ordinary tests.

## Evidence and triage context

- e2e/playwright.config.ts:153 starts start-stack.sh for the suite.
- e2e/start-stack.sh:211 installs a claude wrapper; :226 executes the claude-record fixture; :230–234 prepends that
  directory in the private Bash login profile; :324–326 starts the local supervisor with that HOME and Bash shell.
- crates/farhelm-supervisor/src/service/core.rs:13331 invokes the launch-command builder; :6483 delegates to
  window_command. crates/farhelm-supervisor/src/launch.rs:649–655 selects a login shell; :962–965 builds the agent
  command, and :1000–1008 preserves the inherited PATH after prepending the Farhelm binary directory.
- e2e/tests/real-agent.spec.ts:473–480 submits plain claude. :495–500 requires real Claude readiness and a dismissed
  trust dialog. crates/farhelm-fixtures/src/fake_agent.rs:915 emits FAKE-AGENT READY.
- e2e/tests/real-agent.spec.ts:445-450 enables the manual leg; :473-479 submits invocation claude.
- e2e/tests/real-agent.spec.ts:86-108 enters the command-launch path with that invocation.
- e2e/start-stack.sh:211-228 creates claude as a wrapper executing fake-agent --script claude-record; :230-234 prepends
  the wrapper directory in the private login profile; :324-326 starts the supervisor with that home and Bash.
- crates/farhelm-supervisor/src/launch.rs:649-654 invokes an interactive login shell; :1000-1008 preserves its PATH
  after the product binary directory.
- e2e/tests/helpers/real-agent.ts:105-108 requires the Claude Code v banner; e2e/tests/real-agent.spec.ts:495-500
  additionally requires a trust-dialog dismissal.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:3568–3580 accepts fake agents for ordinary tests while explicitly describing deliberate real-Claude
  execution. It does not accept substituting a fake for an opted-in real-agent leg. No matching Planned item, BUGS
  entry, queue item, or ledger decision was found.
- SPEC_impl.md:3568-3580 distinguishes deterministic fake-agent coverage from an explicitly enabled real-Claude
  exercise; it does not accept substituting the fake in the enabled leg.

Caveats:

- Static verification only.
- The executable-resolution conclusion assumes the intended private login profile operates normally and no claude
  executable has been added beside the Farhelm test binary.
- The isolated HOME also needs deliberate credential handling when this leg is corrected.
- Applies to the configured stack and ordinary binary layout.
- Fails visibly rather than falsely proving real-agent success.
- No runtime test was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_09_sec:p1:F1`,
`test_infrastructure_09_cor:p1:F1`.

- `test_infrastructure_09_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `test_infrastructure_09_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
