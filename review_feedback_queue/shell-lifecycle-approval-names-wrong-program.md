# Agent Stop and Delete approvals name the wrong program for a shell target

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Agent-requested Stop or Delete can ask approval to terminate an agent even though accepting the card terminates the
shell and its work.

## Details

**highest**. Confirmed by source and contract inspection.

Source-confirmed: the supported agent lifecycle rules allow targeting sessions by ID (SPEC.md:4168-4170, :4182-4191).
Helm agent_requests.rs:605-641 resolves the target and, after approval, sends the ordinary Stop with both refusal flags
false. For Delete, :2798-2835 creates the card and dispatches the exact same guard; :2838-2853 obtains Unconditional
when a live shell is included in the request's Stop consent. `ApprovalSession` carries only ID, title and host
(farhelm-proto/src/approvals.rs:79-88). Those are the only target facts in approvals/model.rs:509-510, :697-714. Stop's
summary says `Stop a session's agent and close its terminal tabs` at :245. Delete's `what deletion stops` says
`the agent and all terminal tabs` for Unconditional at :534-546. This is a destructive consent mismatch, with no
wrong-target or extra teardown route. Estimated highest-capable because an action can end a shell and its work after the
card identified the nonexistent agent as the main program being stopped. SPEC.md:1997-2003 explicitly distinguishes the
shell and :2128-2145 limits consent to what was drawn. SPEC.md:4182-4191 requires the agent's card to name the stop
consequence and bind it for Delete. Possible_cover: the older Stop sentence at :4190 uses `its agent` generically but
does not waive the shell and drawn-consent requirements. No proposed drop. Shell-presentation/f1 and shell-contract/f7
address the same mistaken noun on ordinary Delete only. Shell-presentation/f4 is a different environment-specific
confirmation. Follow-up 05ae73f23eb2cbb7ab284ca42473d66a771207a9 fixes ordinary Delete, and no later approval or agent
lifecycle code changes this card through pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. Source analysis only.
Turn-5 interface reconciliation: shell-contract/f21 separately establishes the descendant-liveness gap in guarded Delete
and Replace after the main shell has ended. A fleet agent Delete also derives NothingAlive or AgentEnded from the listed
status and tabs, says respectively `nothing; refuse if the agent or any terminal tab is alive` or
`terminal tabs; refuse if the agent is running again` (agent_requests.rs:2838-2853. Approvals/model.rs:534-546), then
shares the ordinary guarded Delete path (agent_requests.rs:2820-2835. SPEC.md:4182-4184. SPEC_impl.md:7112-7116). Retain
that scope in shell-contract/f21: the same backend guard and descendant sweep make this an additional approval surface
of the existing teardown finding. No separate finding or proposed drop for the underlying termination. It does not
change this record's live-shell wording claim or its severity. This reconciliation also preserves shell-runtime/f14 as
the distinct Stop handler and shell-contract/f19 as its inline Stop and suspend surface.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-permissions/f15`.
