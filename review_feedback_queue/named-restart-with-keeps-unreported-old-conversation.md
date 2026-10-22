# After a named Restart with that cannot report, Resume keeps reopening the previous conversation

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

When you use Restart with to switch a Claude, Codex, Grok, Pi or OMP session into a conversation you name, Farhelm keeps
remembering the session's previous conversation until the agent reports the new one. If that launch cannot report at
all, because you turned Farhelm's conversation hooks off for that agent or Farhelm could not set up its reporter on that
host, the old conversation stays remembered indefinitely. After hours of work in the named conversation, a host reboot
and Resume reopen the earlier conversation instead, and you continue in the wrong one.

## Details

Found by the 2026-10-22 prioritized persistent review, group `109-supervisor-core-11of15` (finding f3, revised).
Original review: `08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7`. Group `A24-relaunch-commit` (f1), the recording agent and
an independent reviewer added the backend handoff and publication trace against the reviewed commit above.

Suggested bucket: highest (resuming the wrong agent conversation after the user moved to another), or high if triage
judges the hooks-off case a deliberate setup.

Confidence: confirmed for the window before a report arrives, which matches the spec. Likely for the never-reports
variant. The relaunch/spawn/publication trace now establishes that the backend does not require a hooked launch or
withdraw old capture when hooking is false. Per-harness policy, successful vendor execution of the named conversation
and observed later Resume into the old one remain unverified. This is source evidence, not runtime confirmation. In
particular, `hooked=false` describes Farhelm's injection decision; it does not prove that no other reporting path can
produce an accepted report. The generic no-report/no-capture sentence alone does not establish that previously valid
capture must be withdrawn.

Mechanism:

- `crates/farhelm-supervisor/src/service/core.rs:11374-11379`: `clear_capture` is true only for named starts of
  harnesses whose `captures_named_conversation()` is false (Goose, Muse, Cursor, OpenCode;
  `crates/farhelm-proto/src/launch.rs:385-402`). For Claude, Codex, Grok, Pi and OMP the captured identity stays,
  whether or not this launch is hooked.
- core.rs:11463-11480 with `crates/farhelm-supervisor/src/store.rs:4072-4170`: `begin_relaunch_named` keeps
  `captured_conversation` unless `clear_capture`.
- A launch can be unhooked: core.rs:2683-2702 (legacy path; the `hooks.allows(kind)` opt-out, where the user's choice
  never notifies) and core.rs:2716-2731 (`HookLog::Skipped`, including an unavailable reporter). The decision to keep
  capture is made at core.rs:11374, before and independently of that hook decision.
- A later plain Restart builds its argv from the durable snapshot (core.rs:11092-11099; `relaunch_argv` at
  core.rs:3307-3341), filling in the old conversation id, and `begin_relaunch_named`'s basis check (store.rs:4120-4124)
  passes because the capture never changed.
- This is the hazard the Goose path clears capture to avoid (launch.rs:387-389: "Keeping its old capture across a named
  restart would leave Restart aimed at a different conversation indefinitely").
- Cross-group answer from `102-supervisor-core-4of15`: the resume target is always this session's own durable capture,
  so it cannot be another session's or a stale read. The only "older conversation" path is this retained-capture window.

Scenario: a Claude session with captured conversation A; the user turns off Farhelm's conversation hooks for Claude (or
the reporter is unavailable on that host); the user does Restart with naming conversation B and works there; the host
reboots; opening the session offers Resume, which runs `claude --resume A`.

The added backend trace closes the earlier untraced-handoff caveat. `service/core.rs:11383-11388` decides capture
clearing from the harness capability before the actual hook result. `relaunch_into_terminal` calls `spawn_agent` with
Resume phase (`11922-11960`) and accepts its result including `hooked` (`11963-11969`). `spawn_agent` computes that flag
(`14395-14396`), records it best-effort (`14513-14520`), dispatches the launch (`14529`) and can return success with the
flag false (`14651-14656`). No reporting requirement gates those paths. Publication builds the Restart offer from the
existing capture (`12484-12490`), while `hooked` only marks the new run's hook state (`12568-12573`); `relaunched_entry`
copies the selected capture (`3019-3055`). No later withdrawal based on false hooking was found in this chain. The
independent reviewer agreed this extends the existing finding while preserving its coverage question.

How to verify: in a service test, capture conversation A for a Claude session, turn hooks off for Claude, Restart with
naming B, and check that the session snapshot still offers A and that a following plain Restart's argv names A.

Fix sketch: decide `clear_capture` after the hook decision, and clear capture (as for Goose) when the named launch is
not hooked; or have the spawn report that it is unhooked so the generation transaction withdraws capture.

Possible cover, for triage to judge:

- `TRIAGE_OUTCOMES.md:13117-13141`, `restart-drain-skips-newly-published-conversation.md`, records the acceptance of
  following an existing capture after tracking silently stops. This is the decision behind the specification below; the
  added backend evidence does not resolve its applicability to a known unhooked named launch.

- SPEC.md "Durability and resume", conversation-identity paragraph (around SPEC.md:2178-2180): "A named Restart with
  keeps the old capture until that report replaces it; Codex reports at the first prompt, so plain Restart before then
  still enters the old conversation." SPEC_impl.md (around 5186): "Reporting harnesses retain old capture until an
  accepted report replaces it." Both premise that a report will come; the unhooked variant, where none can, is not
  obviously covered.
- The same SPEC.md paragraph (around SPEC.md:2185-2186) points the other way: "A launch whose harness cannot report, or
  whose report never arrived, has no captured identity and cannot be restarted: Restart says why, and Replace starts the
  session over." An unhooked named launch that keeps offering the old capture appears to contradict this.
- SPEC.md "Durability and resume", "Confirmed 2026-10-14" (around SPEC.md:2139-2142): "When Farhelm already has a
  conversation recorded and tracking silently stops updating underneath it, continuing to resume that recorded
  conversation is an accepted consequence of whatever bug stopped the updates". This may cover the unavailable-reporter
  case. It reads less naturally on a launch Farhelm knows will never report, and the hooks-off case is a user choice,
  not a bug.
- FILTER.md "Rare edge cases in harnesses without first-class support" does not apply: Claude and Codex are first-class.
