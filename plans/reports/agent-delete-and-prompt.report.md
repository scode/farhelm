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

### Landing

Landed on its own in round 79, as #2192, #2200, #2202 and #2203, merged bottom-up and back to back. They are unchanged
from what the report describes apart from one fix made during the landing (below).

#### What landed since the stack was based, and whether it interacted

Only plan-queue bookkeeping reached main between this stack's base and its landing, so the landing rebase was clean. The
base already held the recent work this stack builds on: the launcher fixes, the per-database agent retry keys, the
non-waiting Claude trust cleanup at supervisor startup, the calm outage card and the WebKit input-method guard. The
landing monitor read the stack against them, and a fresh-context reviewer did the same independently. Neither found an
interaction. The launcher sends the same prompt whether or not it first had to wait for a retry key, the retry keys
themselves are unchanged, the delete path does not use startup cleanup, and the new prompt box keeps the input-method
guard and its own Enter behavior.

The reviewer also checked upgrade compatibility. #2192 raises the protocol to 52, as earlier protocol additions did, and
#2200 adds its prompt fields under the same version; both changelog entries and the report say remote hosts need an
Update. The supervisor's stored data gains no new tables. Its records of creates, kept so a retried create is
recognized, use a new form when the create carries a prompt, and records written before the upgrade still read
unchanged. Because only the whole stack is consistent at protocol 52, the four PRs landed back to back with no release
cut between them.

#### Fix made during the landing: Replace with dropped the initial prompt

The launcher's Replace with dialog offers the initial-prompt box and sends what the user typed. For a replacement in a
folder, the helm forwarded the chosen conversation but created the new session with no prompt, so the user's first
message was silently dropped; a replacement into a managed checkout already kept it. That contradicted the report, which
says a message entered for Replace with is sent to the new session, and the launcher's own design in SPEC_impl.md, which
sends the prompt in Replace with requests. No test covered it.

The landing changed the helm to take the prompt from the Replace with request and pass it to the same create an ordinary
launch uses, which validates it against the agent and includes it in the retry identity. A plain Replace still sends no
prompt. A new helm test, added to #2203, sends a Replace with request carrying a multi-line prompt and checks that the
supervisor receives it exactly. It failed against the code before the fix, run `ae4a31ee-a39a-43a4-883b-9abc9dcf856e`,
on that assertion. A second fresh-context reviewer checked the fix and the test and found no issues.

#### Possible follow-ups

- An empty prompt is sent as a real prompt. It can come from a template that sets an empty prompt on purpose, to clear
  an earlier template's message, or from `--prompt ""`. The agent is then started with an empty prompt argument, and the
  approval card shows an empty message block. How each agent handles an empty prompt argument is unverified. Treating an
  empty prompt as absent when the request is built, while templates keep the empty value as their clearing edit, would
  avoid it.
- The report does not cover downgrading, and a downgrade has two sharp edges. On a downgraded helm, a single template
  carrying a prompt makes the whole template list fail to load, so `tl:` and `--template` stop working until that
  template is deleted. A downgraded supervisor refuses to retry an interrupted fresh-checkout create that carried a
  prompt, which is the safe direction.
- The supervisor's retry record for a create keeps the full prompt text on disk until the session is deleted when the
  create came from the interface or made a fresh managed checkout; a create an agent requested keeps only a hash of it.
- Minor: an oversized `--prompt-file` cut in the middle of a character reports a UTF-8 error instead of the size limit.
  A code comment on the helm's Replace still says a plain Replace recreates exactly the retry identity of the source's
  own create, which is no longer exact when the source started with a prompt or a named conversation.

#### Checks

- Run now, on the stack with the landing fix: the helm's Replace tests, including the new one, through the test recorder
  with the pinned nextest and tmux, four slots and no retries:
  `cargo nextest run -p farhelm-helm --lib -E 'test(replace)'`. 41 of 41 passed, run
  `3e90908b-3226-41ea-934f-b98bf8005fb0`. The new test against the unfixed code, run
  `ae4a31ee-a39a-43a4-883b-9abc9dcf856e`, failed on the intended assertion and then hit the runner's slow-test limit,
  because the test's stand-in supervisor stopped before replying; that is expected for a deliberately broken build.
- Run now: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings` and the isolated `python -B scripts/check-test-sleeps.py` (302 delays,
  none missing a reason), all clean.
- Reused: the executor's recorded runs listed in the report, including its final rebased-tree Rust run
  `9655bee7-7ade-4ff4-9e43-1b6aa4bb6b2e` and browser run `ae221883-1b47-42cc-8c54-29305250b7c9`, both on the base this
  stack landed on, and its builds, Markdown formatting and changelog-format checks. The landing fix changes only the
  helm's Replace path and its tests, which the run above covers, and no Markdown, changelog entry or interface code.
- Skipped: everything the report skipped, for the reasons it gives. No real vendor prompt was submitted, as the report
  says.

Apart from the Replace with claim, which was untrue for folder destinations until this landing fixed it, nothing in the
report became untrue.
