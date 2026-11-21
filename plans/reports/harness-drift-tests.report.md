## What this was about

A new release of an agent CLI can reject Farhelm's launch arguments or stop reporting the conversation ID even while
tests against recorded fixtures pass. This plan adds an opt-in Linux suite for all ten supported agent CLIs. Eight cases
check a real turn, an accepted conversation report, and Restart through a private Farhelm supervisor. Cursor and
OpenCode check that the production launch flags reach an interactive screen or workspace trust prompt. The command is
`python3 -B scripts/run-harness-drift.py`. It runs on an explicit request, including the run-all-tests flow, and each
tracked case that reaches the prompt spends a vendor turn.

## Things you should know

On this host, seven cases passed. Antigravity and Grok could not run because their manual Farhelm hooks were absent.
Muse Code 1.4.24 replied to the prompt. The supervisor trace showed the production launch injected hook flags, but no
conversation report reached Farhelm and there was no hook log before teardown. Farhelm could not offer to resume that
conversation. The test preserves the failure for a separate investigation into why the callback did not arrive.

During acceptance, the vendors' own launchers reported automatic updates. OMP went from 18.22.0 to 19.0.1 and Muse went
from 1.4.14-R7309.1 to 1.4.24-R9131.1. The final tests disable both auto-updaters for their own Start and Restart
invocations. OMP uses a temporary configuration overlay and Muse uses a launcher environment variable. Saved hook files
were untouched. Agents can save trust choices for the temporary test directories.

## Open questions and possible follow-ups

- Diagnose why the current Muse CLI runs a real turn without sending a usable conversation report to Farhelm.
- Verify the Antigravity transcript check and Grok resume on a host where the manual hooks are already installed.
- A successful Restart test proves that the CLI starts with Farhelm's saved resume arguments and accepts fresh composer
  input. A follow-up can check the conversation identity shown after Restart to catch an agent silently opening a
  different conversation. The TODO also covers rotation, subagent reports, prompts, and additional flags.

## Pull requests

- [#4361](https://github.com/scode/farhelm/pull/4361/changes) checks production launches, accepted reports and Restart
  with the installed CLIs.
- [#4364](https://github.com/scode/farhelm/pull/4364/changes) adds the manual runner, retained verdicts, documentation,
  and flow entries.

## Checks

| Agent       | Result                                                | Recorder run ID                        |
| ----------- | ----------------------------------------------------- | -------------------------------------- |
| Antigravity | Could not run. Manual hook absent.                    | `d52afaa4-2953-477d-b888-d297d14570f6` |
| Claude Code | Passed through Restart.                               | `d538b5de-36b9-4b62-934a-287dc228f13f` |
| Codex       | Passed through Restart.                               | `a2b8e4b2-0157-40b6-b742-ccec933e5997` |
| Cursor      | Passed launch smoke.                                  | `a24fde59-7a52-4918-a399-cc941c3854b5` |
| Goose       | Passed through Restart.                               | `e1d9c7e7-fa35-40c1-866e-c3c7f42cd573` |
| Grok        | Could not run. Manual hook absent.                    | `70e7c664-99c7-412b-86b7-5d40227dad85` |
| Muse Code   | Failed after a reply. No conversation report arrived. | `1e5abdb8-1d09-4dea-bf76-4677b4e7ab2e` |
| OMP         | Passed through Restart.                               | `810afec7-ac47-47b9-aec7-38586f0bef2a` |
| OpenCode    | Passed launch smoke.                                  | `1bf61d14-f32e-403d-8307-06dd07865275` |
| Pi          | Passed through Restart.                               | `ce2ca47c-1a00-48ec-ba5d-174314fa0e31` |

- The full live run used the runner before its signal forwarding and recorder diagnostic fixes. Focused reruns of
  Claude, OMP and Muse exercised the final startup corrections. The Codex, Cursor, Goose, OpenCode and Pi passes come
  from the `c824f96a` driver candidate. Subsequent changes adjust the Claude trust prompts, OMP and Muse startup,
  diagnostics for an exited process, and remove an assertion about the next Restart offering resume. Those changes do
  not invalidate the successful turns and composer observations.
- The actual runner's `--list` found ten cases. Explicit Antigravity and Grok selections each reported COULD NOT RUN and
  exited 1. The nine stand-in tests passed through the recorder in `a62dc7e1-7d84-4d03-8f33-a7d0b1c3b98f`.
- After rebasing onto the Antigravity trust and model catalog changes on main, two existing stand-in e2e tests passed
  through nextest in `9a36a09e-44c2-4e56-a90b-63e9ac2b5f05`. They checked a structured wrapper through Start and Restart
  and a wrapped Muse reporting and resuming its conversation. The rebase keeps the tested model IDs and CLI arguments.
  The Antigravity drift case still uses the default trust setting.
- Focused `cargo clippy --locked -p farhelm --test e2e -- -D warnings`, `cargo fmt --all -- --check`, dprint on the
  changed Markdown, the Rust test sleep audit, and `git diff --check` passed. The instruction checker found zero errors
  and one existing archived citation warning. Root `AGENTS.md` is 28,535 bytes, under the 28 KiB limit.
- The routine batteries and the existing ignored Claude `/clear` and Codex `/new` cases were skipped. This stack adds
  separate cases and uses existing helpers without editing their behavior. Rerunning the older live cases would spend
  additional turns on unchanged coverage.

## Review gate

The required single source reviewer checked each PR. Corrections covered launch flags, startup and auth detection,
diagnostics before teardown, cancellation through the recorder, and preservation of the recorder’s own failure reason
from its documented manifest schema. The reviewer confirmed the final targeted fixes. Scope review kept Restart
validation on the existing supervisor and recorder paths.

### Landing

#### What landed with it

Nothing else landed in this round. Since this plan's work started, main gained the Antigravity workspace trust switch,
the launcher's model family renames, template placeholder checks on save, automatic host updates skipping staged builds,
and the v0.43.0 release notes draft. An independent reviewer and I read this plan against them. Every model the tests
pin is still offered, the separate shortcut word the launcher's model list used to carry is not used here, and
Antigravity's launch is unchanged while its new trust switch is off, which the test leaves at its default.

The reviewer also checked that the new tests are safe to run on a shared machine. They use private temporary state, no
fixed ports or names, never change their own environment, write no hook files, and record environment variable names but
never their values, so no credentials reach the evidence. The OMP and Muse update overrides apply only to the tests' own
launches.

#### Fixes made while landing

One went in as a third PR, [#4372](https://github.com/scode/farhelm/pull/4372/changes), which changes only instruction
text:

- The "run all tests and fix" instructions had lost the rule against running a test battery that the flow's deflake
  sweep runs anyway; it is back. Their new step for this suite said only to keep going on a failure, while the rest of
  the flow wants every failure fixed or recorded. A failed agent case is now fixed when that is straightforward and
  otherwise recorded in TODO.md as a change in the vendor's program, not as a flaky test, without holding up the rest.
- The suite's guide told the reader to sign in and install hooks themselves. Agents follow that guide when they run the
  suite, and they must not sign in, accept vendor updates or edit hooks, so it now says an agent reports such a case as
  "could not run" instead.

#### Checks

The plan adds opt-in tests that run the real agent programs (ignored by every default run), a runner for them with its
own tests that use stand-ins, a guide, and entries in the agent instructions. The live suite itself spends a real vendor
turn per agent and runs only on request, so the landing did not run it; the executor's report has its per-agent results.
The landing ran the main lint (clippy), which compiles the new tests, on a separate arm64 Linux test machine, and the
runner's own tests and the small checks locally on x86_64.

On the plan as delivered, stacked on main:

- `cargo clippy --all-targets -- -D warnings`, on the test machine, not through the recorder: passed.
- `cargo nextest run -p farhelm --test e2e -E 'test(/real_agent_capture::/)'`, on the test machine (recorder run
  `930b8238-acd6-4973-8bc1-221e3fb1779c`): the test program built, and it listed all ten new agent cases as skipped
  because they are opt-in. The run selected no test, since every test in that group is opt-in, and the recorder fails a
  run that selects nothing. The recorder therefore lists that run as failed; that is expected, and the run is evidence
  that the cases stay out of default runs, not a test result.
- `python3 -B scripts/test-run-harness-drift.py` (the runner's own tests, no vendor turns), locally: all 9 passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 scripts/check-agents-md.py` (formatting, test and instruction lints), locally: passed.

The landing fix (#4372) changes only agent instructions and the suite's guide: `dprint check` and
`python3 scripts/check-agents-md.py` passed. Skipped: the workspace suite, since no code that ordinary tests run
changed; the app's tests and the end-to-end smoke test, since no app code changed; and Mac runs, since the suite is
Linux only.

#### Things you should know that the report does not say

- Muse is the one agent that failed: it answered the prompt, but no conversation report reached Farhelm. If that holds
  for the current Muse release in ordinary use, Farhelm cannot offer to resume a Muse conversation, so Restart may not
  return to it. The report lists the diagnosis as a follow-up; it may be worth doing soon.
- The Antigravity case answers Antigravity's "trust this directory?" question instead of using the new trust switch, so
  each run adds its temporary directory to your Antigravity trust list. Turning the switch on in the Antigravity test
  would avoid it, but that could not be checked, since the Antigravity hook is not installed on the machine the plans
  run on.
- The runner gives each case 480 seconds, about 20 seconds more than the test's own time limits add up to, so a slow run
  could be cut off before the test writes its own failure details. Raising that limit would be a small follow-up.
- The check for which agents need a hook you install yourself is written per agent by name in the test, rather than
  through the per-agent tables that the rest of Farhelm's code uses for such decisions, with a catch-all that stops the
  test for any other agent. A new agent that needs a hook would hit that stop at run time instead of being asked for a
  decision when the code is built. It is test code only.

#### What you should still do

Nothing for this landing. To get Antigravity and Grok coverage, install their hooks on a Linux machine and ask for "run
real harness tests" there.
