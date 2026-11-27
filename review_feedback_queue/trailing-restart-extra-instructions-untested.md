# No supervisor or end-to-end test pins that a Restart's blocked-agent instruction reaches the relaunched agent

Reviewed commit: 93294dee7dd395d28ec12f86294891c594015773

## TLDR

SPEC.md says sessions restarted while blocked alerts are on get the blocked-agent instruction. The helm side is tested
(it sends the instruction on RestartSession), and the create path is tested end to end, but nothing checks that the
supervisor actually carries a Restart's instruction into the relaunched agent's command line. If the supervisor dropped
it on Restart (for example a future refactor of the relaunch plan passing None), restarted agents would silently stop
being told to run `farhelm agent notify`, and every test would still pass.

## Details

The commit threads `extra_instructions` from `ControlMsg::RestartSession` through `RestartSessionRequest`
(handlers.rs:2124-2283), `restart_session_named` (core.rs, the `extra_instructions: Option<String>` parameter),
`RelaunchPlan.extra_instructions` (core.rs:4170 struct, built at core.rs:12938), `relaunch` -> `relaunch_into_host`
(host_restart.rs:223-280) -> `spawn_agent_in_host` -> `prepare_launch`. Every supervisor test touching these
(handlers.rs tests at 4307, 5046, 9379, 11371, 11709, 11726, 12245; core.rs restart tests; host_restart.rs:1465;
core.rs:38705; claude_restart.rs:164; e2e restart_with_resume.rs:569) passes `extra_instructions: None`. The only tests
with Some(..) are: the create e2e `extra_instructions_reach_the_injected_claude_start_hook_and_print_after_the_pointer`
(hook_identity.rs, create only), the validate_retry unit test (core.rs:17155, create retry only),
agent_kind/mod.rs:3147/3331 (pure expansion), and the helm's sessions_tests.rs:3997-4018 (asserts the helm SENDS it on
RestartSession, against a fake supervisor peer). Replacing `extra_instructions` with `None` at the RelaunchPlan
construction in restart_session_named, in `relaunch`'s call to `relaunch_into_host`, or in relaunch_into_host's call to
spawn_agent_in_host compiles and passes every test. Verify by grepping the tree for `extra_instructions: Some` and
`Some(` near restart calls. A fix is one supervisor test (or an extension of the existing hook_identity e2e) that
restarts a Claude-kind session with Some(extra) and asserts the relaunched SessionStart hook command carries
`--extra-instructions` with that text.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other (test-coverage gap; current code carries the value correctly by source reading).
- Confidence: confirmed (by source: no test passes Some extra instructions through RestartSession on the supervisor
  side).
- possible_cover: none known
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g23/f1 (finding), group g23 "Asking agents to notify when blocked: supervisor and hook CLI", findings log line 1.
