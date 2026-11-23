## What this was about

Farhelm could stop tracking the conversation in Muse Code 1.4.24. Both Muse hooks still ran, but Muse stripped the
Farhelm variables the hooks used to find the report directory. Without a saved conversation ID, **Restart** could no
longer resume it.

Farhelm now passes the non-secret session ID and supervisor socket path in the two Muse hook commands. The hook uses
those values to find the supervisor's private report directory. The supervisor checks the hook's recorded process
ancestry against the current launch before accepting its report. The session token stays in the environment for
authenticated requests and out of the command line.

## Things you should know

The fix applies when Farhelm builds a new Muse launch. An earlier launch with no saved conversation ID may still need a
manual resume if you know its ID.

Muse Code 1.4.24-R9131.1 ran offline with its echo provider. Both callbacks wrote the same conversation ID with an empty
Farhelm environment. Separately, the Linux integration tests drove the real Farhelm supervisor through capture and
**Restart**, using a fixture that proves it removes the launch variables from both hook children and checks that the
actual launch arguments omit the generated session token.

## Open questions and possible follow-ups

The optional post-landing `drift_muse` check would verify that a live interactive Muse turn is captured and that Restart
resumes it. It spends vendor turns. The offline callback and fixture evidence covers the plan's acceptance criteria.

## Pull requests

- [#4696](https://github.com/scode/farhelm/pull/4696/changes) restores Muse Code conversation capture for Restart,
  updates the hook documentation, and removes the completed TODO.

## Checks

- Passed on `9ca4f61c` on Linux. Rebuilt `farhelm-fixtures` and ran
  `cargo nextest run --locked -p farhelm --bin farhelm --test e2e` for the `agent_client`, `hook`, `hook_identity`, and
  `structured_launches` modules. All 92 selected tests passed with no runtime skips (four slots, no retries, recorder
  `8d85534b-f9ed-420b-a786-d8109f18c042`).
- An earlier run of this selection on `1acfdbae` failed in two assertions because the tests used a credential lookup
  expecting a different form of session ID (retained recorder `d7e15219-edb4-4e29-8573-4446d698c0e2`). After correcting
  the lookup, both `a_two_level_resident_wrapper_captures_and_resumes_*` cases passed on `9ca4f61c` with
  `cargo nextest run --locked -p farhelm --test e2e` restricted to those two tests (four slots, no retries, recorder
  `363381f0-e99a-4830-840e-28ed42be5818`).
- Passed on `9ca4f61c`. A local Deno probe called
  `muse exec --provider echo --json --no-foreign-personal-context --disable-web-tools --disable-reminders -c <hook-config> <prompt>`
  with Muse Code 1.4.24-R9131.1. Both callback writes passed with zero vendor turns (one process, recorder
  `d018064c-3128-4182-b6f1-c3146de0fa20`). `cargo clippy --locked -j 4 --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, the test-sleep check and changelog format also passed. The draft has the same source
  tree.
- Reused `cargo nextest run --locked -p farhelm-supervisor --lib` for the 14 Muse injection and shared hook policy tests
  on `1acfdbae` (four slots, no retries, recorder `496ee460-8054-4431-af1f-147301e58f14`). Only the CLI end-to-end
  test's lookup changed after that run. Reused the website `bun run build` and link check from `feb23e03` (recorder
  `ce219c5f-8538-46bd-b54a-f6fc03698afa`) for the same edited page. Markdown formatting and diff whitespace checks
  passed.
- Skipped GUI captures and the full runtime suite. The patch affects hook commands and reporting, and the focused tests
  exercise both a fresh launch and Restart. Live Muse drift was skipped because it spends vendor turns.

## Review

The plan's single Sol high reviewer checked the patch and verified the corrections. The review strengthened evidence
that both callbacks lose the variables, checked the actual Start and Restart arguments for the session token, kept
explicit socket paths within the existing UTF-8 contract, and corrected the command documentation. The final correction
used the supervisor's local session ID for the token assertion. The reviewer found no remaining issues.
