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

### Landing

#### What landed with it

It landed alone in its round, and main had gained nothing but plan bookkeeping since the stack was based, so nothing
else could interact with it. Your TODO entry about SSH password prompts (#4278) merged during the landing and touches
only TODO.md.

At your request this landing was reviewed with extra skepticism, by me and by an independent reviewer, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The code is proportionate:
Antigravity reuses the way Farhelm already works out which process reported for Claude and Muse, rather than adding
machinery of its own, and every per-agent decision sits in the per-agent places the repository's code rules require, not
in shared code. Two gaps needed fixing before it could land (the first two items below), and several texts read badly
(the rest). I also looked at the desktop app: the Antigravity chip sits after Claude, the Guarded help reads as below,
and a launched session shows the new mark and starts `agy --mode auto`. At a 900-pixel-wide window the agent chips now
take two rows (OpenCode moves to the second); they wrap cleanly, and at 1100 pixels they still fit on one, matching the
executor's check.

#### Fixes made while landing

- **The instructions agents read left Antigravity out.** `farhelm agent instructions` listed the agents that accept a
  first message without Antigravity, so an agent asked to start Antigravity with one would have refused or avoided
  `--prompt`. It also left Antigravity out of what each agent's Guarded mode does. Both lists now include it, with the
  fallback to Antigravity's default mode where auto mode is unavailable.
- **One early exit skipped the reply Antigravity expects.** When the hook command could not take the lock on the
  installed Farhelm version (the Mac app holds it during an update or uninstall), it exited without replying.
  Antigravity reads every hook's output as a JSON reply, and the specs promise one on every path. What Antigravity does
  with an empty reply is unknown; it may show a hook error. That exit now writes the same empty JSON object. No test
  covers this path; it needs the Mac app's installed layout.
- **The correction notice** for Antigravity said "Restart would have resumed that conversation", which reads as if
  Restart still would. It now says "A Restart before now would have resumed that conversation", as the Claude, Codex and
  Muse notices do.
- **The Guarded help** in the launcher said "Farhelm requests auto mode. When available, Antigravity runs commands in
  its sandbox…". It now describes the mode first and then says that where auto mode is unavailable on the account or
  host, Antigravity uses its default mode. The Supported agents docs table says the same; its row had given no
  mechanism, unlike every other row.
- **The Antigravity docs page** gained what the Grok page already tells users: where to look when Restart never becomes
  available (`/hooks`, then the session's hook log), that the hooks do nothing outside Farhelm, and that Antigravity is
  not told about `$farhelm` requests. Its subagent caveat now says the main conversation takes Restart back when it next
  reports.
- **A comment in the hook code** said its reply "writes one line", which is true only for Antigravity, and had lost why
  the instructions pointer is the one deliberate output for the other agents. It says both again.
- **SPEC_impl.md** named only "a second Grok started inside a Grok session" as a nested run whose report can briefly
  take a session's report slot before being refused. Antigravity's hooks are global too, so any `agy` run inside another
  Farhelm session (a Claude running `agy -p`, say) can do the same; the example now says so. The next report repairs it,
  as before.

#### Checks

The plan adds an agent type across the protocol crate (a protocol version bump), the helm's launch builder, the
supervisor, the command line's hook command and its end-to-end tests, and the desktop app, so the landing ran the whole
Rust workspace suite, the app's end-to-end smoke and the reference screenshots (goldens). Builds ran locally; the
workspace suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plan as delivered, rebased on main:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `python3 -B scripts/test-gui-agent.py`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy --locked -p farhelm-gui --all-targets --features agent -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and the website build
  (`cd website && bun install --frozen-lockfile && bun run build`): passed.
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats
  (`27ed094d-6bfa-422d-be59-66057ab7dc98`, `4bffb619-a26d-422c-9cb1-ba09a3fc2759`,
  `1dc1d672-22ac-4813-a341-822bac4be178`): 8093 of 8094 passed, with no runtime skips. The macro compile-contract test
  failed as it always does on a boat, since it resolves crates from the local cargo cache, and passed locally
  (`cargo nextest run -p farhelm-testtrace-macros`, `97692894-600c-4796-bd61-2d3157703b23`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed, and the clipboard
  paste step was skipped, as always on Linux (`80599b8e-bc22-48bf-9f5d-83481f99f5d9`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`405305ef-9ca6-4e11-947b-ad3c39788024`). No reference
  screenshot shows the agent chips or the new mark.

On the fixed tree:

- `cargo fmt --all -- --check`, `dprint check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings` and the website build: passed.
- The agent instructions, hook, Antigravity, notification, pin and GUI permission tests,
  `cargo nextest run -p farhelm -p farhelm-supervisor -p farhelm-gui --bin farhelm --lib --test e2e -E 'test(/agent_instructions|hook::|antigravity|notifications::|claude_pin|permission::/)'`,
  locally: 77 passed (`4903bc82-648e-490f-924f-d9841f890066`).

The rest of the workspace suite, the smoke and the goldens were reused for the fixed tree. After them only text changed
(two lists in the agent instructions, a notice, the Guarded help, a comment and docs) plus one early exit's reply; the
focused run covers the instructions, the notice and the hook, and the changed help line appears in no reference
screenshot.

Skipped:

- The macOS desktop build and its tests, and the version-lock exit's new reply, which runs only in the Mac app's
  installed layout: nothing ran on a Mac.
- A live run of Farhelm with a real Antigravity CLI: the landing machine has none, and the executor's earlier probes ran
  Antigravity alone, not under Farhelm (see below).

#### Things you should know that the report does not say

- The workaround for a subagent taking over Restart is to enter the main conversation's ID in the launcher, but neither
  Farhelm nor these docs say where Antigravity shows that ID, and Farhelm does not show which conversation it follows.
  Possible follow-up: a sentence on the Antigravity page saying where `agy` shows its conversation ID.
- Farhelm and a real Antigravity CLI have still never run together; the report says so too. One risk to check when they
  do: Farhelm skips `sh`, `bash` and `dash` when working out which process reported. If Antigravity runs hooks through
  another shell (zsh on a Mac, say), each report would be credited to that short-lived shell. Restart would still work,
  but the protection against a nested Antigravity replacing the conversation would quietly stop applying. Possible
  follow-up: one live run with the documented hooks file on Linux and on a Mac.
