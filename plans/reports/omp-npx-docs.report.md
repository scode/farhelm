## What this was about

The OMP documentation promised that Farhelm could follow conversations started through `npx`. On Debian, Ubuntu and
other Linux systems whose `sh` is dash, the agent runs but Farhelm cannot follow its conversation, leaving Restart
unavailable. The maintainer chose to correct the documentation without changing how launches are recognized.

The [OMP page](/docs/agents/omp/) now distinguishes running the installed `omp` command, `bun x` and the compiled binary
from this `npx` limitation. Its source comments and implementation notes distinguish argument-pattern tests from live
npm behavior. The corresponding documentation TODO entry is removed.

## Things you should know

This changes documentation and comments only. It does not make `npx` conversation tracking work. Launches elsewhere,
including macOS, remain explicitly untested. There is no changelog entry because this changes no released behavior.

## Open questions and possible follow-ups

The existing statement that a resident wrapper always leaves Restart greyed out could not be confirmed from the code. A
single native or shell wrapper at the terminal's top process may be accepted; extra intermediary processes are refused,
and a Node launcher is refused for a direct `omp` launch. The plan explicitly allowed retaining that sentence and
reporting this gap. A follow-up can verify the relevant wrapper launch shapes and narrow the statement if needed. No
live wrapper or macOS `npx` verification was performed here.

## The PRs

- [#3207: correct OMP npx support claims](https://github.com/scode/farhelm/pull/3207/changes) — one draft PR based on
  main.

## Checks run, reused and skipped

- Passed `dprint check SPEC_impl.md TODO.md website/src/content/docs/docs/agents/omp.md` and
  `cargo fmt --all -- --check`.
- Passed `bun install --frozen-lockfile` and `bun run build` in `website`: 36 pages built and all internal links valid.
  The build reported warnings for the empty i18n collection and missing 404 content, then completed successfully.
- These checks covered revision `1c2c43ede8c66805954ef222f8835078fc0456ac`; the draft commit
  `8e2dde49355d02d2236f5d39ca3c58861064e6aa` has the same file contents. No additional checks were reused from another
  run.
- Skipped runtime tests, clippy, browser tests and CI because only prose and comments changed. No test or fixture
  changed, so no test-sleep check was needed. No instruction file or heading changed, so no agent-reference check was
  needed. No preview server was started because this is an unattended plan.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort passed the change with no actionable findings. It
identified the retained wrapper statement's verification gap described above. The independent commit-title cold read
matched the intended documentation correction and found no contradiction or convention violation.
