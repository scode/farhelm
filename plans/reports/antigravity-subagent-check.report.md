# Keep Antigravity Restart on the main conversation

## What this was about

Stopping or restarting Antigravity while a subagent was active could make **Restart** open the subagent's conversation.
Both conversations report from the same Antigravity process, so their reports passed the same process check. You chose
to keep automatic Restart and check whether an incoming report could be tied to the main conversation.

In Antigravity CLI 1.5.13 on Linux, the main conversation's reported log started with user input. A subagent's log
started with a system message once written. Farhelm now checks that first record before changing what **Restart** opens.
The docs explain the check and how to find the conversation ID with Antigravity's **General** tab in `/help` to open it
directly.

## Things you should know

The live probes covered a typed first prompt, a startup prompt, a resumed conversation with a startup prompt, and
`/clear` in a running CLI. The main log was readable at every measured callback, including both start and end events
where emitted. Three new subagents reported a missing log at startup and a system record at Stop. The resumed
conversation reused a subagent and reported its system record at Stop. All observed child callbacks also had a child
file under the parent. The named log was sufficient and works before a parent is known. No measured main callback needed
to be recovered by a later Stop. The four usable traces took six short prompts, including two repeats to collect the
missing timing evidence from the first prompted and typed runs.

The probe recorded normalized paths. The end-to-end fixture exercises an absolute path, as specified by the CLI's hook
contract. The eight named files observed with records were regular files when checked again.
[Antigravity CLI hooks](https://antigravity.google/docs/hooks#managing-hooks-in-antigravity-cli)

A missing or unreadable log, an unrecognized first record, or a first line past 64 KiB leaves the previous Restart
target in place. A later verified report can take effect. An incompatible CLI update can make Farhelm ignore every new
report without a notification. A session that already captured a subagent before this fix needs a report from the main
conversation to correct the saved target.

## Open questions and possible follow-ups

If the main conversation changes during Restart, Farhelm can stop the current agent and refuse to open its replacement.
The session can be restarted again. Refusing child reports removes one way to hit this race. The remaining case is
already queued for review and needs no decision here.
[Restart race review](https://github.com/scode/farhelm/blob/main/review_feedback_queue/antigravity-restart-may-stop-without-relaunching.md)

A child report can still replace a waiting main report before Farhelm processes either, such as during a supervisor
outage. Farhelm keeps the last accepted conversation in that case until the main conversation reports again.

## Pull requests

- [#4326](https://github.com/scode/farhelm/pull/4326/changes) verifies Antigravity's main-conversation reports before
  updating Restart.
- [#4327](https://github.com/scode/farhelm/pull/4327/changes) explains the check and how to find a conversation ID.

## Checks

The final Linux run passed all 20 selected tests (`b5032351-8d98-461d-80e8-4cddbf69f907`). It used the repository test
recorder with four nextest slots and this selection.

```sh
cargo nextest run -p farhelm -p farhelm-supervisor --lib --bin farhelm --test e2e \
  -E 'test(antigravity) | test(claude_pin::)'
```

The end-to-end fixture used controlled transcripts to drive the real Farhelm hook and Restart, exercising valid, child,
missing, malformed and oversized records. The first build overlapped the parser correction. Its updated fixture linked
the earlier supervisor and showed the array-shaped record being admitted (`34f71ffb-67db-4ad8-a042-53844c0ed16f`). The
final build rejected it. The final run covers the code in #4326. Main moved only by adding an unrelated TODO entry
before the draft opened, and the docs draft changes no executable code. No other runtime results were reused.

The website built 38 pages and all internal links passed. Rust formatting, Markdown formatting, test delay rationales,
changelog format and instruction links passed. The instruction checker emitted four existing citation warnings.

The full workspace and macOS checks were skipped. The changed path is platform-independent report admission, covered by
the focused Linux fixtures. The live Antigravity CLI timing and log observations were on Linux only.

## Review

Both changes passed the independent source review and cold wording check. The source review tightened the
malformed-record check and pinned tests to the actual hook outcomes. The docs review aligned the hook troubleshooting
and Restart guides with what Farhelm verifies when it receives a report.

### Landing

#### What landed with it

It landed in the same round as remember-active-terminal, which changes only the app's terminal tabs; nothing else that
is not bookkeeping reached main since this plan was based. An independent reviewer and I read the two stacks against
each other and found no interaction.

The review confirmed three things. The check follows the repository's rule that agent-specific behavior lives in each
agent's own code, not in shared code. It runs before Farhelm saves anything about which conversation Restart opens. When
a subagent finishes, Farhelm accepts its report silently, but it refuses the report sent when a subagent starts, as
SPEC.md says. Antigravity support shipped in v0.42.0, so the separate "fixed" changelog entry is right.

#### Fixes made while landing

These went in as a third PR, [#4332](https://github.com/scode/farhelm/pull/4332/changes).

- **The hook log docs misdescribed the new subagent refusal.** The hook log now shows a refusal each time a subagent
  starts. The docs page that explains hook log lines said such a conflict means a report from some other agent, which
  reads like something went wrong. The page now says this refusal is expected and means Restart stayed on the main
  conversation.
- **SPEC_impl.md said the next verified report "restores" Restart's target**, which is wrong when Farhelm had not
  recorded a target yet. It now says that report sets the target again, or for the first time.

#### Checks

The round's two plans change the app's terminal tab selection (remember-active-terminal), and how the supervisor decides
whether to accept an Antigravity report, Farhelm's Antigravity hook, its end-to-end test and the docs
(antigravity-subagent-check). So the landing ran the whole workspace suite, the end-to-end smoke and the two lint
(clippy) runs the validation guide calls for on the maintainer's own test machine (an arm64 Linux VM that builds and
tests the tree itself), and the reference screenshots, the website build and the small checks locally on x86_64.

On both plans as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test machine
  (`de032654-f2c8-4111-941c-fcbcf26020ae`): all 8154 passed, with no runtime skips. This covers both plans' Rust,
  including the app's tests of the restored tab and the Antigravity hook's end-to-end test.
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on the test machine
  (`84428fb7-59aa-43a8-8871-4e9269a5203f`): 49 of 50 steps passed, the clipboard paste step skipped as always on Linux.
  Its steps open terminals, type into them and switch between sessions.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test machine,
  not through the recorder: passed.
- `python3 scripts/check-gui-goldens.py`, locally (`4677417e-55f9-4e4d-88bf-fcd2fe294035`): passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and the website build
  (`bun run build`), locally: passed.

The round's landing fixes (#4331 for remember-active-terminal, #4332 for antigravity-subagent-check) change one code
comment, two specification sentences and one docs paragraph, so the runs above were reused for them; after the fixes,
`cargo fmt --all -- --check`, `dprint check` on the edited files and the website build passed again locally.

Skipped: the desktop app's own tests and its lint runs (neither plan touches the desktop app's code; the smoke builds
and runs it), anything on a Mac, and a live Antigravity CLI, which only the executor's probes covered.

#### Things you should know that the report does not say

- The CLI behavior the check rests on (the main conversation's log starts with your input and is readable at every
  report; a subagent's is missing or starts with a system message) comes from the executor's probes of Antigravity CLI
  1.5.13 on Linux. Neither the landing review nor any check here reran them.
- The check has no test of its own. The end-to-end test covers the valid, subagent, missing, malformed and oversized
  cases through the real hook. No test covers a report that names its log by a relative path; the CLI's hook contract
  specifies absolute paths, and the code refuses a relative one and leaves the Restart target unchanged.
