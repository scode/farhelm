# Agent deletion and initial prompts

## What this was about

Agents could stop sessions but could not delete them. New sessions also had to wait for someone to type the first
message into the terminal. This plan adds approved agent deletion and optional initial prompts through the CLI, session
launcher and launch templates, following the maintainer's decisions. The four PRs form one linear draft stack; the
executor has neither marked them ready nor merged them.

## Things you should know

`farhelm agent delete --session ID` deletes an ended session. A running agent or open terminal tab requires
`--stop-if-running`. The user approves the operation unless that host already allows acting commands without asking. The
approval binds what may be stopped and which host connection was inspected; a changed target is refused. Deletion uses
the same cleanup as the UI, including denying waiting approval cards, clearing session marks and reporting any
managed-checkout trash notice. Replace reuses the common successful cleanup too. If a reply is lost, deletion may
already have happened; the missing reply establishes neither success nor failure.

Self-delete is refused. Two agents trying to delete each other cannot wait forever on each other's active request: agent
deletion refuses a busy target instead. The UI's existing waiting behavior is unchanged. Allowing self-delete later
remains the separate Maybe later item the planning work added.

`farhelm agent create` and ordinary `farhelm spawn` accept mutually exclusive `--prompt TEXT` and `--prompt-file PATH`;
`-` reads stdin. Prompts may contain newlines and tabs, count toward the existing 64 KiB launch budget, and refuse other
control characters. The full escaped prompt appears on an agent's approval card. Older UIs fail to decode a
prompt-bearing card rather than approve an unseen first message.

The launcher offers the multiline initial-prompt box only for supported agents. Enter adds a line. Recent setups leave
the current draft alone; Clone and Replace-with prefills clear it. Templates can supply and save a prompt, including a
deliberate empty edit that clears an earlier template's message. Invisible formatting from a template is escaped and
read-only until cleared. The exact raw message participates in retry identity, including managed-checkout requests.

The message is supplied once when the new agent process starts and is absent from the session's reusable launch and
recent history. Restart, Clone and Replace do not replay the old message. A new message explicitly entered for a
Replace-with launch is sent to that new session. Each fresh launch from a prompt-bearing template uses its message.

Installed CLI help and, where necessary, parser inspection support Claude, Codex, Cursor, Grok, Muse and OMP positional
prompts after `--`, and OpenCode's `--prompt=TEXT`. Goose is unsupported because its composed interactive command has no
confirmed safe prompt input. Pi is unsupported because its parser still treats leading `@` text as a file attachment
after `--`. Muse refuses prompt plus named conversation. `spawn --inherit-agent` refuses both prompt flags rather than
adding a second inherited-prompt mechanism. No real vendor prompt was submitted during validation.

Protocol 52 covers both features. Remote hosts need an Update after upgrading Farhelm. The stack has two feature
changelog fragments, and the last PR removes both completed TODO entries. Documentation changes cover Start a session,
Launch templates and Approve what agents do.

## Open questions and possible follow-ups

No maintainer decision is required to finish this plan. Self-delete remains deferred as requested. Goose or Pi support
would need a verified interactive command-line input that preserves arbitrary literal text. The screenshots in the
existing documentation were not recaptured; this stack updates their surrounding feature text.

## The PRs

- [#2192: let agents delete sessions](https://github.com/scode/farhelm/pull/2192/changes), based on main.
- [#2200: accept initial prompts on session creation](https://github.com/scode/farhelm/pull/2200/changes), based on
  #2192.
- [#2202: edit initial prompts in templates](https://github.com/scode/farhelm/pull/2202/changes), based on #2200.
- [#2203: supply initial prompts in the launcher](https://github.com/scode/farhelm/pull/2203/changes), based on #2202.

## Checks run, reused and skipped

Runtime selections used the recorder, pinned nextest 0.9.148 and pinned tmux, with four Rust slots or one browser worker
and zero retries. Successful output and reports were complete, with no runtime `SKIPPED` marker.

- Deletion: 223 affected Rust cases passed in `5933d011-bb6b-43c3-8b8b-22370e412dbb`, including CLI, relay, approval,
  guards, self-delete, outcome-unknown and request-fence coverage. The corrected five-case delete selection passed in
  `b326f487-7dda-41ce-afaa-5dd518fe7d3e`. These cover deletion commit `147fc42f`; its unchanged behavior was reused
  after rebase, with the final trust-cleanup interaction checked below.
- Prompt transport: 286 affected Rust cases passed in `1b14c84f-6fe0-4c58-ab48-b2b0bac2fb98`, covering protocol,
  templates, approval compatibility, CLI/relay, literal argument placement, refusals and pending/completed retry
  identity. Final stdin fixture passed in `8a8db4fe-a500-4c93-8f72-37b5f2491717`; final ordinary/inherited spawn cases
  passed in `4a155831-396c-42bd-9abd-4126460db441`. Evidence covers the product tree in `febad61c`, whose transport
  behavior is unchanged by the rebase.
- Template editor: 18 focused Rust cases passed in `9db3c3bc-81da-438d-99c7-de82188b7ffd`; four browser cases passed
  across both engines in `295062ab-f1bc-4a7a-9a63-f3d001cd9afa`. This covered `5fa1ba33`; final combined tests below
  also exercise that editor after rebase.
- Final rebased tree: 60 focused Rust cases passed in `9655bee7-7ade-4ff4-9e43-1b6aa4bb6b2e`, including launcher/API,
  template/save behavior, deletion fences and deferred/startup trust cleanup. Sixteen browser cases passed, eight per
  engine, in `ae221883-1b47-42cc-8c54-29305250b7c9`: capabilities, Enter, literal request retries, template stacking,
  save selection, recent/Clone/Replace behavior and template editing. Desktop and phone captures were inspected.
- Final `cargo fmt --all -- --check`, `dprint check`, both required Clippy configurations, changelog format, test-sleep
  checker (302 delays, zero missing rationales) and whitespace checks passed. Native and Dioxus web builds passed. The
  website build passed before the rebase and is reused: the changed page is identical, and upstream's documentation edit
  is on a separate page with no changed link dependency.
- The careful rebase onto `169931ea` inspected all incoming diffs. The textarea retains native Enter under the global
  composition guard; post-delete trust cleanup retains its twelve-second budget despite startup becoming nonwaiting;
  prompt composition remains separate from bare Git cache operations. No conflicting product decision was found.
- Earlier failures remain retained: deletion `18c52167-e634-4e5b-8d74-b5194832c3d0` exposed a wrong HTTP status
  assertion and an unclosed lost-reply fixture; prompt `8450051e-38ec-40bc-b275-148389b49063` exposed the stdin
  fixture's use of a runner that deliberately clears stdin. Compilation-only correction runs
  `38f671b0-a435-46b9-9bc1-f7bd0300d264` and `36c13165-1c59-480a-a2ce-13b47fe2095c` failed before runtime. All were
  corrected and covered by later distinct runs; they were same-session test/compile defects, not recorded as latent
  flakes.
- Broader workspace, full browser, installer, provisioning, desktop runtime and doctest runs were skipped because the
  targeted evidence covers the changed contracts, no desktop-specific seam or executable doc example changed, and no
  additional material risk warranted those gates. The live installation was untouched.

## Review gate's outcome

Each PR passed the required fresh-context gpt-6.1-sol/high shellout source review after local corrections. Findings
included literal-input hazards for Pi/OMP, preserving empty and dormant template prompts, and browser locator/fixture
mistakes. The final review reported no remaining concrete finding; the required scope reassessments found no unnecessary
mechanism. Source review is distinct from the recorded runtime evidence above.

All four commit/PR wordings passed cold reads. Implementation stayed local under no-workhorse mode. Review usage is
partially recorded privately; native and orchestrator usage was unavailable, and shellout JSON did not echo actual model
identity. This report passed a separate native cold read for clarity and public-repository hygiene.
