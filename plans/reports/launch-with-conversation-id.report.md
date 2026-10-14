### What this was about

When Farhelm had not captured an agent's conversation, a user who knew its id could not tell Restart to enter it. The
maintainer wanted the same choice wherever a launch dialog appears, without changing what New, Clone, Replace with or
Restart with otherwise does.

Those dialogs now accept a conversation for this start only. The ordinary saved launch remains unchanged, so recent
setups, defaults and templates do not save the selector and a later Clone starts fresh. The agent's report still
establishes the captured conversation; entering an id never establishes identity by itself.

### Things you should know

All nine installed agents were checked against purpose-created conversations through their real TUIs. Each resumed
screen showed the prior conversation; Goose showed its resuming indication, conversation name and restored context. The
offering agents and labels are:

| Agent    | Field           | Verified version   | Verified named form                                                                        |
| -------- | --------------- | ------------------ | ------------------------------------------------------------------------------------------ |
| Claude   | conversation id | 2.1.299            | `claude --permission-mode default --resume <conversation-id>`                              |
| Codex    | conversation id | 0.162.1            | `codex --no-daemon --dangerously-bypass-approvals-and-sandbox resume <conversation-id>`    |
| Grok     | conversation id | 1.0.52             | `grok --no-leader --resume <conversation-id>`                                              |
| Goose    | session id      | 1.50.1             | `env GOOSE_MODE=approve goose session --resume --session-id <session-id>`                  |
| Muse     | conversation id | 1.4.7              | `muse --trust-workspace --provider echo resume <conversation-id>`                          |
| Cursor   | chat id         | 2026.10.14-0f9edde | `cursor-agent --trust --resume <chat-id>`                                                  |
| OpenCode | session id      | 1.18.31            | `opencode --session <session-id>`                                                          |
| Pi       | session file    | 0.85.1             | `pi --no-approve --session <session-file>`                                                 |
| OMP      | session file    | 18.10.4            | `omp --approval-mode write --resume <session-file> --no-extensions --no-skills --no-rules` |

Commands above redact only the purpose-created ids and file paths; private test configuration and session-directory
inputs are omitted. Muse used its installed TUI and session store with the echo provider. Cursor reported an older
version on the initial start and the version above on resume, without an explicit update request. All test TUIs were
stopped. No live Farhelm installation or user-level vendor configuration was changed.

Claude, Codex, Grok, Pi and OMP can report after a named start. A named Restart with keeps the old captured conversation
until an accepted report replaces it. Codex reports at the first prompt, so plain Restart before that prompt still
enters the old conversation.

Goose does not attach its reporter on resume. A named Goose restart therefore forgets its old capture, so later Restart
cannot silently enter the wrong conversation. A definitive failure before an external start restores the old capture; an
ambiguous failure does not. Named Goose, Muse, Cursor and OpenCode starts explain that later plain Restart will be
unavailable. Muse, Cursor and OpenCode still have no ordinary resume command and do not gain Restart with; they have the
field in New, Clone and Replace with.

For an agent launch with a stored resume command, Restart with stays reachable when capture is missing or unusable. Then
the selector is required. A supplied target enables the primary action even if the ordinary settings are unchanged.
Working-agent stop consent, fixed agent type, folder identity and ordinary launch validation remain enforced.

Values pass literally as one argument. The existing strict conversation-id grammar refuses spaces, quotes, backslashes,
controls, leading dashes and reserved tokens. Pi and OMP accept absolute or relative session files; relative paths
resolve in the session's folder, and a leading tilde is refused. Farhelm does not check existence: the agent handles an
unknown id or file. A command launch refuses a separate selector, and a new managed checkout refuses it; choose an
existing folder instead.

The supervisor protocol is now 50 because an old supervisor ignoring this field would start the wrong conversation. This
release requires remote hosts to be updated. The docs website pages were updated and built, not deployed.

### Open questions and possible follow-ups

No maintainer decision is needed for this implementation. Session files containing spaces remain unsupported under the
existing validator. Conversation discovery, a CLI selector, template persistence and adding reporting to generic agents
remain outside this plan.

### The PRs

- [#2012: start sessions in a named conversation](https://github.com/scode/farhelm/pull/2012/changes), draft, pushed
  head `bc7808a046618967be849e3628639915a9f44cba`. Backend and UI are one coherent review unit. Nothing was marked ready
  or merged by the executor.

### Checks run, reused and skipped

Recorded Rust runs used pinned nextest and tmux in an owned bounded Linux container, four global slots and zero retries.
Browser runs used the pinned Playwright runner, one worker, zero retries, Chromium and WebKit. Private run evidence and
screens are named in `farhelm-plan-launch-with-conversation-id-log.md` beside the checkouts.

- Run: `cargo check` for the affected backend and UI during implementation; the later test builds and Clippy cover the
  corrected Rust source. `cargo fmt --all -- --check`, targeted `dprint check`, `git diff --check`, changelog format and
  the isolated `python -B scripts/check-test-sleeps.py` passed. The delay checker inspected 298 delays, none lacking a
  rationale.
- Run through the recorder:
  `cargo nextest run -p farhelm-helm -p farhelm-supervisor -p farhelm-ui -p farhelm-proto --lib -E 'test(named) | test(restart_with)'`.
  Runs `a459ede9-26db-4a16-be41-a8ea81f0d5e3`, `b754f336-b9c5-479c-98d7-8829fb307754` and
  `f71a9bad-bdce-4d7e-8455-364f05354793` passed 26, 31 and 31 selected cases respectively as coverage was added and
  review fixes applied. The latest run covers argv, request boundaries, retry identity, capture withdrawal/restoration,
  labels and no-capture availability; 2,790 cases were excluded by its filter.
- Run through the recorder: `cargo nextest run -p farhelm-supervisor --lib -E 'test(named_start_refuses)'`;
  `2c650093-2f06-4dce-a9b0-8af51113b825`, one passed. This specifically verifies the final malformed-shape and
  byte-bound refusals, with the size fixture otherwise valid and a bound-specific assertion.
- Run through the recorder:
  `cargo nextest run -p farhelm --test e2e -E 'test(named_restart_with) | test(named_create_runs_once) | test(restart_with_claude_yolo)'`;
  `9965b67e-a307-43c2-aa07-b85f1ef3e78a`, four passed. Real private tmux proves literal argv, old-to-new reported
  capture, fresh cloning, idempotent named creation and refusal without stopping the original session. The existing YOLO
  restart regression passed too.
- Run: `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, both passed. The
  second checks the shipped configuration without test seams. Source/lint failures during development were corrected
  before these final passes.
- Run: the Rust product/fixture build and explicit Dioxus web release build for browser prerequisites. The final web
  build passed after the field-width correction. `bun install --frozen-lockfile && bun run build` passed for the
  website, with valid internal links.
- Run through the browser recorder: `npx playwright test named-conversations.spec.ts restart-with.spec.ts`;
  `4b84da34-defd-44bc-9358-743e91a8bfd9`, 58 passed and two failed. Both failures were the new no-capture fixture's
  missing stored resume command. Its existing restart focus, consent, draft/refusal and Enter cases passed on both
  engines.
- Run through the browser recorder: selected named, launcher-tab, retry-key and value-control Enter cases;
  `0b5509fe-a17b-4534-bcca-9ace43deb443`, ten passed and two failed. Both failures were an expectation of an unset trust
  field that the ordinary wire shape omits. The existing launcher cases and named label/New request cases passed.
- Run through the browser recorder: `npx playwright test restart-with.spec.ts -g 'named restart with'`;
  `b3644789-c8bf-465e-8eee-83b3476c91dd`, two passed after the fixture and expected wire shape were corrected.
- Run through the browser recorder:
  `npx playwright test named-conversations.spec.ts restart-with.spec.ts -g 'named conversation|New sends a named|named restart with'`;
  `565fc30a-9748-4f37-a8f4-48a9889099cc`, six passed after final layout polish. Executor inspection of retained desktop
  and phone captures confirmed a full-width readable field without horizontal clipping, on both engine families.
- Reused across careful rebases: the successful named behavior checks, final Clippy, web and website builds, from the
  work based on `9d27ead52a90f48671bbf159c21579ab3afe5da6` through the final base
  `48491e78c703bd01ae777d351f3f419c5aa6605c`. Incoming product work adds the help-menu Release notes item; its Feedback
  specification and docs changes are separate from launcher, admission, capture and consent. Other incoming changes are
  TODO, planning, report and review-instruction bookkeeping. Rebase had no conflict or behavioral interaction requiring
  another runtime run. The final named layout run uses the rebuilt selector; the equivalent lint cleanup does not
  invalidate prior behavior coverage.
- Skipped: full workspace/browser batteries, hosted CI, desktop shell smoke, installer/provisioning/release gates and
  website deployment. The change has focused execution and browser coverage; it changes no desktop entry point,
  installer, release artifact or provisioning behavior. No new executable examples require doctests.

Failed observations were retained separately. Rust runs `87c01caa-1481-4bfd-a567-6d74dd6b6569` and
`851e71fa-0735-4bf5-8827-9de9f51647a1` exposed a record-only report fixture, incorrect wrapper generations and
overlapping transcript needles; those test errors were corrected. Browser runs `0fec8622-580d-404d-81f4-865e6abf0f59`
and `251785ff-f12c-4b78-8922-2e0554046860` were interrupted after browser-executable setup refusals. The preinstalled
engines were then made available in the container's private cache. Mixed browser runs above remain failures, not full
passes; later successes are separate observations. No same-session failure was added to the public flake ledger.

### Review gate outcome

The required single fresh-context gpt-6.1-sol/high reviewer found two implementation issues: malformed transient argv
could bypass the ordinary placeholder-shape rules, and the two session-file checks duplicated harness decisions. Both
were corrected by reusing the shared shape checks and an exhaustive capability. Its follow-up found that the oversized
fixture now failed an earlier shape check, so it was made otherwise valid and required the bound-specific refusal. The
same reviewer approved the final source and narrow layout/rebase delta, with no remaining findings. It performed source
review, not runtime tests or visual verification. The executor collected the execution and capture evidence above.

The separate scope review accepted the transient request design while requiring named retries, resume-phase hooks and
Goose's durable/publication capture state to agree. Those obligations are implemented. A separate documentation pass
covered every touched file. Commit and PR wording passed a fresh cold read; report delivery has its own native cold
read. Implementation and investigation stayed local under no-workhorse. Only required scope, correctness, wording and
report reviews were delegated. Private galaxy-brain records retain routing/outcomes and available external usage;
initial scope accounting was delayed, and native/orchestrator usage is unavailable. No cost comparison is claimed.
