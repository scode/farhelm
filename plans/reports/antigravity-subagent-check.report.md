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
