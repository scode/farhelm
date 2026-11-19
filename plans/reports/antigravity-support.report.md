# Antigravity support

## What this was about

Farhelm now starts Antigravity CLI from the session launcher or with `farhelm agent create --agent antigravity`. The
launcher offers Gemini models and effort, Guarded, YOLO and Custom permissions, an initial prompt, and a conversation ID
for opening an existing conversation.

To set up Restart, install the two hooks from the Antigravity guide under the account that runs Antigravity, in each
host or environment where you run it. For a fresh conversation, Restart becomes available after your first prompt, once
Antigravity reports the conversation.

## Things you should know

Antigravity CLI 1.5.12 reported both the main conversation and a subagent from the same process. The callbacks carried
different conversation IDs and no marker identifying the child. The landing monitor answered on the maintainer's behalf
and kept automatic Restart to preserve the agreed launch and tracking behavior. Restart can open a subagent's
conversation if that subagent reported last. To open the main conversation directly, enter its ID in the session
launcher's Conversation ID field.

Guarded requests Antigravity's auto mode. Antigravity uses its default mode when auto mode is unavailable on the account
or host. The session list shows basic running and idle status. You answer Antigravity approvals in its terminal.

## Open questions and possible follow-ups

A reliable child marker would let Farhelm keep automatic Restart on the main conversation. A follow-up can turn off
automatic Restart for Antigravity while keeping launch support. Opening an existing conversation would then require
entering its ID.

## Pull requests

- Add Antigravity launches, conversation hooks, Restart and desktop controls.
  [#4274](https://github.com/scode/farhelm/pull/4274/changes)
- Document setup, permissions, and the Restart caveat, and remove the TODO entry.
  [#4275](https://github.com/scode/farhelm/pull/4275/changes)

## Checks run, reused and skipped

- `cargo nextest run` passed 242 selected tests across the protocol, launch compiler, hook attribution and Restart, and
  the GUI (`50cc09a7-85fa-470c-9fd9-333d55cd9c2c`). The end-to-end tests exercised successful and failed callbacks and
  the actual Restart destination using an isolated stand-in for Antigravity and the real Farhelm hook and supervisor.
- Passed `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the Rust test delay check, the changelog checker, and Markdown
  formatting. The instruction link checker passed with four existing warnings. The website built and all internal links
  validated after review fixes.
- `python3 -B scripts/test-gui-agent.py` passed 93 staging tests (`f001b7fb-a749-4f74-9ac6-8c3a3f428c4b`). The SVG
  generator produced the Antigravity mark, and the focused Rust tests verified the bundled image.
- `python3 scripts/check-gui-goldens.py --update` captured all 268 Linux reference screenshots. Every screenshot matched
  the existing reference (`9291986e-564c-49e3-848d-94d340a0188e`). The later prompt and help wording changes are outside
  what these reference scenes show.
- Opened the actual launcher in a private headless Linux desktop build of the final source. All ten agent choices fit in
  one row at 1100 pixels. At 776 pixels they wrap to two rows, and scrolling exposes the whole lower row
  (`1bd6a8dd-88af-4dcc-a750-893bd63c914d` for both widths, `d4ea502e-f981-468d-8ee2-3bb2f493ce41` for narrow scroll).
- Reused live Antigravity CLI 1.5.12 probes from the previous claim on base `7f9e0e32`. These isolated `agy` runs
  established first-prompt timing, same-process subagent reports, and that an explicit ID can be combined with an
  initial prompt. The probes measured Antigravity behavior independently of the Farhelm changes and have no test
  recorder ID. A separate review probe confirmed the CLI needs the attached flag spelling for a literal prompt such as
  `--help`.
- The focused tests exercised the changed Linux launch, callback and GUI paths, so the full runtime battery was skipped.
  Live CLI probes and the stand-in integration test cover the two sides of Restart. A full run with both Farhelm and the
  live Antigravity CLI together remains unverified. Native macOS validation was skipped because the desktop checks ran
  locally on Linux. The docs-only PR was checked by the website build and formatting checks.

## Review gate

Fresh gpt-6.2-sol high reviewers checked both drafts. The code review found that option-like initial prompts could be
interpreted as CLI flags. The fix keeps the prompt in one literal argument, and the reviewer verified it. The docs
review corrected Auto mode fallback, which account and environment need the hooks, the notification and child-report
contracts, and troubleshooting. Both PRs had separate commit-message cold reads.
