# Restart wait hints: say when Restart becomes available, per agent

Written against main at bd9d6643 on 2026-10-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file directs a change to them.

## The goal

While a session's agent reports conversations but Farhelm has not yet captured one it can resume (the `not_captured`
restart offer, `RestartOffer::NotCaptured` in `crates/farhelm-proto/src/lib.rs`), Restart and Restart with are greyed
out. Their hover text today says "no conversation Farhelm can resume was captured for this session, so replace starts it
over". That reads as final, and it is in Farhelm's terms rather than the user's. For most agents it is a normal,
temporary state: Codex, for instance, reports its conversation only once the user submits the first prompt.

After this plan, every surface that explains a `not_captured` offer says, for the session's own agent and naming that
agent, when Restart normally becomes available, or, once the session has ended, that its conversation was never
captured. The app's text adds a parenthetical inviting the user to send feedback if it does not.

The maintainer's words, from the planning conversation: "for each harness can we give the user (depending on the agent
type) the expected behavior (e.g. "For codex agents, this operation becomes available when..." - and naming the harness
so its clear it's tailored)." On the cases where the conversation will never arrive (which the app cannot tell apart
from waiting; see the outline): "primarily state the expected behavior. then state in parenthesis that if this isn't
working, please submit feedback". The maintainer also asked for the supervisor's refusal message and the agent
instructions to carry the per-agent wording, and confirmed the wording below.

The source TODO.md entry is "Explain unavailable Restart in the user's terms". Its example ("if Codex reports its
conversation only after its first turn") is inaccurate: Codex reports when the first prompt is submitted. Use the facts
in the table below, verified during planning.

Acceptance criteria:

- **When Restart becomes available, per agent kind** (the facts the wording states; re-verify each against the sources
  cited in the outline before writing it, and correct this table in the log as a DECISION if one is wrong):

  | Agent kind | Restart becomes available                                                                   |
  | ---------- | ------------------------------------------------------------------------------------------- |
  | Claude     | a few seconds after it starts                                                               |
  | Goose      | as soon as it starts                                                                        |
  | Codex      | once the user submits the first prompt                                                      |
  | Pi         | after the agent's first reply, once the conversation is saved                               |
  | OMP        | after the agent's first reply, once the conversation is saved                               |
  | Grok       | after the first prompt, and only if Farhelm's Grok hooks are installed (name the docs page) |

  Grok's pointer to its hook setup is plain text (a docs page name, or a bare URL if that reads better): none of these
  surfaces can hold a link. Mentioning `/new` and `/clear` (after which Codex, Pi and OMP capture again) is optional;
  leave it out unless it fits without lengthening the text noticeably. Kinds with no conversation reporting (Muse,
  Cursor, OpenCode, which run as generic, and plain commands) never reach `not_captured` in practice; they keep today's
  `no_conversation_reporting` text. An agent kind the app does not recognize, or a row carrying no kind (an older helm),
  gets generic wording that makes no per-agent claim, plus the same feedback parenthetical.

- **The app, while the session is running** (`crates/farhelm-ui/src/session_view.rs`): for `not_captured`, the Restart
  control's tooltip and accessible description read like this for Codex, with the agreed shape for every kind:

  > For Codex sessions, Restart becomes available once you submit your first prompt; until then, Replace starts the
  > session over. (If Restart doesn't become available, please send feedback from the ? menu.)

  Name the help menu the way the app labels it to the user (the `?` button at the top of the sidebar, `app_bar.rs`);
  check its visible label and accessible name and use what a user would recognize. The Restart with control's greyed-out
  reason for `not_captured` (today "no captured conversation to resume") carries the same per-agent timing in its own
  short form. Its reason function (`restart_with_reason`) folds `NotCaptured` and `NoResumeCommand` into one branch
  today; split it so `no_resume_command` keeps its own text.

- **The app, once the session has ended** (exited, errored, or interrupted by a host reboot): the agent is gone, so the
  timing sentence would be false. These get final wording instead (Decision 7), for Codex:

  > Farhelm never captured this Codex session's conversation, so Restart can't resume it; Replace starts the session
  > over. (If you expected Restart here, please send feedback from the ? menu.)

  This applies to the tooltip, accessible description and Restart with reason of an ended session, and to the
  interrupted session's card. The tooltip and the interrupted card are built from one shared clause today
  (`offer_clause`, used by `restart_offer_text` and `interrupted_surface_text`); they must keep sharing one source, now
  chosen by whether the session is running. The interrupted card's existing composition (status lead, Replace with
  naming) keeps working. Which statuses count as "running" (including starting, waiting, unknown) is the executor's call
  from `SessionStatus`; log it.

- The other offers' (`no_conversation_reporting`, `no_resume_command`) text is unchanged everywhere.

- **The supervisor's refusal** of a Restart or Restart with request on a `not_captured` session states the same
  per-agent timing, without the feedback parenthetical (Decision 4), and still names Replace. Both refusal paths must
  give it: `RestartOffer::unavailable_reason` (used by `relaunch_argv` and the Restart with path in
  `crates/farhelm-supervisor/src/service/core.rs`), and the earlier Codex/Grok-specific check
  (`agent_kind::unverified_resume_refusal`, called in `service/core.rs` before both paths whenever the offer is not
  `Resume`), which today answers a never-prompted Codex session with "its legacy identity is unattributed, or its exact
  record is unavailable". For a `not_captured` session that check must not pre-empt the new reason; keep its own wording
  for the cases it was written for (verify which offers those are before changing its condition, and log it). The
  refusal may keep the running/ended distinction or not; the supervisor knows the session's status, so final wording for
  an ended session is preferred if it falls out naturally.

- **The agent instructions** (`crates/farhelm/src/agent_instructions.rs`, the `not_captured` explanation): say,
  compactly and per agent kind, when `not_captured` normally turns into `resume`, with no feedback note. Generate the
  per-kind part from the same timing function (looping over the kinds) rather than hand-writing a second table. Keep "Do
  not restart such a session; tell the user instead."

- SPEC.md's sentence on greyed-out Restart (session header section: "their tooltips and accessible descriptions explain
  the specific reason, such as ... or a conversation that was never captured") is updated to say that for an agent that
  reports conversations, a running session's explanation says when Restart becomes available for that agent, an ended
  one's says the conversation was never captured, and the app's version invites feedback.

- No change to when Restart is offered, to the wire protocol, or to the supervisor's capture logic.

- The last PR removes the TODO.md entry "Explain unavailable Restart in the user's terms".

## Decisions already made

Settled with the maintainer during planning. Do not reverse them.

1. **Expected behavior first, feedback in parentheses.** The app cannot tell "still waiting" from "will never arrive"
   (see the outline), and the maintainer chose not to build that distinction: state the expected behavior, then the
   feedback invitation. Rejected: a new restart-offer value or field from the supervisor (protocol bump, and it would
   collide with the in-flight `session-notifications.md` plan's changes to the same capture code).
2. **Name the agent**, so the user can see the text is tailored ("For Codex sessions, ...").
3. **Keep the short Replace clause** ("until then, Replace starts the session over"). The code's own rule is that every
   unavailable clause names Replace; the maintainer confirmed the wording with it.
4. **Three surfaces**: the app's text (tooltip, accessible description, interrupted card, Restart with), the
   supervisor's refusal message, and the agent instructions. The feedback parenthetical is in the app's text only: the
   refusal and the instructions are read by agents through the `farhelm` CLI, and a pointer to the app's help menu means
   nothing to them.
5. **Review gate:** two fresh-context reviewers per PR, Claude Opus 5.5 at high effort and gpt-6-astra at high effort,
   no review swarm (How to run, Review gate).
6. **No-workhorse mode** (How to run).
7. **Ended sessions get final wording**, not the timing sentence, as quoted in the acceptance criteria. Chosen by the
   maintainer after the planning review pointed out that the timing sentence is false once the agent is gone.
8. **No dependency on the feedback deployment.** In-app feedback cannot reach the maintainer until its endpoint is set
   up (a separate TODO.md entry); until then a send fails with the dialog's ordinary failure message. The maintainer
   chose to land this plan regardless.

## Implementation outline

Planner proposals unless marked as a decision. Grounded in main at bd9d6643.

**Why the app cannot tell waiting from never (verified during planning).** `RestartOffer` has only unit variants, and
`NotCaptured`'s doc lumps together a report that has not arrived yet, a harness not set up to report (hook injection
turned off, Grok without its user-installed hooks, a legacy launch where injection was skipped), a refused report
(attribution failures), ownership-version gates, and verification that withdrew Resume. `SessionInfo` carries nothing
that separates these. That is why the text hedges (Decision 1). Do not add a wire field. The session's status is the one
distinction the app does have, and Decision 7 uses it.

**Sources for the timing table.** `IntegrationSnapshot::restart_offer` in
`crates/farhelm-supervisor/src/agent_kind/mod.rs` decides the offer; per-kind identity rules live beside it. Claude's
`SessionStart` hook and Codex's first-prompt report: SPEC_impl.md near "conversation hook" (around the sentences on when
each harness reports), `agent_kind/mod.rs`'s note that a Codex session "created but never prompted never reports at
all", and `crates/farhelm/tests/e2e/real_agent_capture.rs`. Goose's MCP reporter: `crates/farhelm/src/goose_hook.rs` and
SPEC_impl.md. Pi and OMP: `crates/farhelm-supervisor/assets/pi-conversation-v1.ts`, `omp-conversation-v1.ts`, and the
website's Pi and OMP pages. Grok: SPEC.md's Grok paragraphs and the website's Grok page (whose title is the docs page to
name for its hooks). The website's `agent-hook-injection` page summarizes several. The table was assembled from these;
confirm it.

**Where the per-agent wording lives.** Root `AGENTS.md` "Harness-specific code": no `kind == X`, `matches!` over
specific kinds, or `_` arm over kinds in shared code; per-kind facts go in the places the module docs of
`crates/farhelm-supervisor/src/agent_kind/mod.rs` name, and a new place is added to that map. Proposal: one exhaustive
function on the proto's `AgentKind` (`crates/farhelm-proto/src/lib.rs`, beside `AgentKind::word()`) returning only the
timing clause for each kind and the agent's display name (`None` for `Generic`), carrying
`#[warn(clippy::wildcard_enum_match_arm)]` like the UI's `SessionAgentKind` impl does. Each surface builds its own
sentence around that clause, so there is no second per-kind table (no separate short form for Restart with). The
supervisor's refusal, the agent instructions and the UI all use it. The UI's tolerant mirror `SessionAgentKind`
(`crates/farhelm-ui/src/lib.rs`, next to `drag_copy_hint`) maps to the proto kind, with `Unrecognized` falling back to
generic wording; the UI already depends on `farhelm-proto`. Add the new place to the map in `agent_kind/mod.rs`. If the
code argues for a different split, take it and log the DECISION, but keep one source of the timing facts.

**The refusal.** `RestartOffer::unavailable_reason` takes no kind today. Give the refusal paths the session's kind (the
supervisor's snapshot holds it) without changing `RestartOffer` itself or the wire, and handle the
`unverified_resume_refusal` ordering described in the acceptance criteria. Check every `unavailable_reason` call site in
`service/core.rs` and log what each does.

**Agent names.** "Claude", "Codex", "Goose", "Pi", "OMP", "Grok": what the app's `launch_composer::harness_label` and
the website's page titles use. Confirm and keep them in the one function above.

**Tests.** Update the session view's unit tests that pin the old wording
(`the_offer_text_states_what_would_happen_to_the_conversation`, `the_interrupted_surface_matches_the_restart_offer`,
`restart_with_availability_explains_each_unavailable_case`) to check the running and ended text per kind and the generic
fallback, without pinning every sentence verbatim. One unit test for the timing function covering every kind. A
supervisor test that a never-prompted Codex session's Restart refusal carries the new reason, not the
`unverified_resume_refusal` text. The Playwright spec `e2e/tests/terminal-restart.spec.ts` asserts the old tooltip text
on an injected interrupted row with no kind; update its expectation to the ended, generic wording. A new Playwright
assertion is optional. Check the agent CLI's tests that may pin the instructions' text.

**Interaction with other plans.** `session-notifications.md` is in flight and changes the supervisor's capture tripwire
and adds per-session notifications; it may also touch the session view. No ordering is needed, but if it lands first,
rebase per root `AGENTS.md` "Careful rebase" and keep the two texts from contradicting each other.

**Size.** Small to moderate. Suggested stack: one PR, a `feat` carrying a changelog fragment. Split only if the PR grows
awkward (the proto function with the supervisor and CLI changes, then the UI, say), without churn.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-restart-wait-hints-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/restart-wait-hints/<nn>-<short-name>`.
- Shape the stack per the outline. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted
  in a later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits. The change is user-visible and carries a changelog fragment under `releasing/changelog.d/` in
  the same commit, per root `AGENTS.md` (Releases and the changelog). Validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge (`plans/AGENTS.md`).
- Changes to tests or fixtures follow `.agents/test-authoring.md`; run `python -B scripts/check-test-sleeps.py` per root
  `AGENTS.md` when Rust or browser tests change.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, nextest selections of the proto crate, the UI's session view, the
supervisor's restart refusal tests and the agent CLI tests that pin instruction text, the Playwright
`terminal-restart.spec.ts` on Chromium and WebKit (after the builds root `AGENTS.md` lists), and `dprint check` on
changed files. Say in the report which checks ran and why.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands (Decision 5): a Claude Opus 5.5 agent at high effort, and a gpt-6-astra
agent at high effort (shelled out to the other harness if the executing one cannot reach that model natively). No review
swarm. Both get the same prompt, which carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory, one per reviewer), and the acceptance criteria: The goal, Decisions already made, and
the outline above. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim. Address what both reviewers find before moving on, and log the DECISION where you decline a finding. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a wire or protocol change, a new restart-offer
value, a change to when Restart is offered, a per-kind table duplicated across crates; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the request, the decisions above, this outline, the current diff and
the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular any correction to the timing table, which statuses count as running, where the
per-kind function lives and how the UI and supervisor reach it, the agent names used, the exact wording per surface, and
every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. The planner proposals in the outline are yours to change when the code argues otherwise. Not
covered, and a reason to block per `plans/AGENTS.md` (Executing one plan, step 10) after recording the concrete
tradeoff: changing the wire protocol or the restart offer's values, dropping one of the three surfaces, adding the
feedback invitation to the refusal or the agent instructions, using the timing sentence for an ended session in the app,
or a timing fact that turns out to be unknowable for a kind (block on that kind's wording only if no honest,
still-useful sentence exists; otherwise log it).

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has removed the TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report
through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
